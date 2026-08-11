//! Stable C ABI bridge between MoonBit and OpenDAL.

#![deny(unsafe_op_in_unsafe_fn)]

mod abi;

use std::collections::HashSet;
use std::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::slice;
use std::str;
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use abi::*;
#[cfg(feature = "layers-timeout-retry")]
use opendal::layers::{RetryLayer, TimeoutLayer};
use opendal::options::{
    DeleteOptions, ListOptions, ReadOptions, ReaderOptions, StatOptions, WriteOptions,
};
use opendal::raw::PresignedRequest;
use opendal::{BytesRange, Capability, EntryMode, ErrorKind, Metadata};
use tokio::runtime::Runtime;

const MAX_OUTPUT_BYTES: u64 = i32::MAX as u64;
const WHOLE_READ_CHUNK_BYTES: usize = 1024 * 1024;
const BINDING_VERSION: &str = env!("CARGO_PKG_VERSION");
const OPENDAL_VERSION: &str = "0.58.1";
#[cfg(feature = "profile-standard")]
const SERVICE_PROFILE: &str = "standard";
#[cfg(not(feature = "profile-standard"))]
const SERVICE_PROFILE: &str = "local";
const LAYER_FEATURE_BITS: u64 = if cfg!(feature = "layers-timeout-retry") {
    FEATURE_LAYERS
} else {
    0
};
#[cfg(feature = "layers-timeout-retry")]
const OPERATOR_LAYER_TIMEOUT: u32 = 1 << 0;
#[cfg(feature = "layers-timeout-retry")]
const OPERATOR_LAYER_RETRY: u32 = 1 << 1;

#[cfg(feature = "profile-standard")]
const PROFILE_FEATURE_BITS: u64 = FEATURE_S3;
#[cfg(not(feature = "profile-standard"))]
const PROFILE_FEATURE_BITS: u64 = 0;

static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();

#[cfg(test)]
const TEST_LISTER_NEXT_ERROR: u8 = 1;
#[cfg(test)]
const TEST_LISTER_NEXT_PANIC: u8 = 2;
#[cfg(test)]
static TEST_LISTER_NEXT_TARGET: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static TEST_LISTER_NEXT_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

#[cfg(test)]
const TEST_READER_READ_ERROR: u8 = 1;
#[cfg(test)]
const TEST_READER_READ_PANIC: u8 = 2;
#[cfg(test)]
static TEST_READER_READ_TARGET: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static TEST_READER_READ_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

#[cfg(test)]
static TEST_WHOLE_READ_OBSERVATIONS: Mutex<Vec<(usize, u64)>> = Mutex::new(Vec::new());

#[cfg(test)]
const TEST_WRITER_WRITE_ERROR: u8 = 1;
#[cfg(test)]
const TEST_WRITER_WRITE_PANIC: u8 = 2;
#[cfg(test)]
const TEST_WRITER_CLOSE_ERROR: u8 = 3;
#[cfg(test)]
const TEST_WRITER_CLOSE_PANIC: u8 = 4;
#[cfg(test)]
const TEST_WRITER_ABORT_ERROR: u8 = 5;
#[cfg(test)]
const TEST_WRITER_ABORT_PANIC: u8 = 6;
#[cfg(test)]
static TEST_WRITER_CALL_TARGET: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static TEST_WRITER_CALL_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
#[cfg(test)]
static TEST_WRITER_NATIVE_CLOSES: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static TEST_WRITER_NATIVE_ABORTS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
fn install_lister_next_test_mode(lister: *mut ListerV1, mode: u8) {
    use std::sync::atomic::Ordering;

    TEST_LISTER_NEXT_MODE.store(mode, Ordering::Relaxed);
    TEST_LISTER_NEXT_TARGET.store(lister.addr(), Ordering::Release);
}

#[cfg(test)]
fn take_lister_next_test_mode(lister: &ListerV1) -> u8 {
    use std::sync::atomic::Ordering;

    let address = ptr::from_ref(lister).addr();
    if TEST_LISTER_NEXT_TARGET
        .compare_exchange(address, 0, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        TEST_LISTER_NEXT_MODE.swap(0, Ordering::Relaxed)
    } else {
        0
    }
}

#[cfg(test)]
fn install_reader_read_test_mode(reader: *mut ReaderV1, mode: u8) {
    use std::sync::atomic::Ordering;

    TEST_READER_READ_MODE.store(mode, Ordering::Relaxed);
    TEST_READER_READ_TARGET.store(reader.addr(), Ordering::Release);
}

#[cfg(test)]
fn take_reader_read_test_mode(reader: &ReaderV1) -> u8 {
    use std::sync::atomic::Ordering;

    let address = ptr::from_ref(reader).addr();
    if TEST_READER_READ_TARGET
        .compare_exchange(address, 0, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        TEST_READER_READ_MODE.swap(0, Ordering::Relaxed)
    } else {
        0
    }
}

#[cfg(test)]
fn install_whole_read_observer(operator: *mut OperatorV1) {
    let mut observations = TEST_WHOLE_READ_OBSERVATIONS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    observations.retain(|(address, _)| *address != operator.addr());
    observations.push((operator.addr(), 0));
}

#[cfg(test)]
fn observe_whole_read_chunk(operator: &OperatorV1, length: u64) {
    let mut observations = TEST_WHOLE_READ_OBSERVATIONS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, observed)) = observations
        .iter_mut()
        .find(|(address, _)| *address == ptr::from_ref(operator).addr())
    {
        *observed = observed.saturating_add(length);
    }
}

#[cfg(test)]
fn take_whole_read_observation(operator: *mut OperatorV1) -> u64 {
    let mut observations = TEST_WHOLE_READ_OBSERVATIONS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    observations
        .iter()
        .position(|(address, _)| *address == operator.addr())
        .map(|index| observations.swap_remove(index).1)
        .unwrap_or(0)
}

#[cfg(test)]
fn install_writer_call_test_mode(writer: *mut WriterV1, mode: u8) {
    use std::sync::atomic::Ordering;

    TEST_WRITER_CALL_MODE.store(mode, Ordering::Relaxed);
    TEST_WRITER_CALL_TARGET.store(writer.addr(), Ordering::Release);
}

#[cfg(test)]
fn take_writer_call_test_mode(writer: &WriterV1) -> u8 {
    use std::sync::atomic::Ordering;

    let address = ptr::from_ref(writer).addr();
    if TEST_WRITER_CALL_TARGET
        .compare_exchange(address, 0, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        TEST_WRITER_CALL_MODE.swap(0, Ordering::Relaxed)
    } else {
        0
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct StructHeaderV1 {
    struct_size: u32,
    struct_version: u32,
}

enum CallFailure {
    AbiMismatch,
    Error(ErrorV1),
}

type CallResult<T> = Result<T, CallFailure>;

fn abi_mismatch<T>() -> CallResult<T> {
    Err(CallFailure::AbiMismatch)
}

fn binding_error(kind: u32, name: &str, message: impl Into<String>) -> CallFailure {
    let mut message = message.into();
    if u64::try_from(message.len()).map_or(true, |len| len > MAX_OUTPUT_BYTES) {
        message = "error message exceeds the binding output limit".to_owned();
    }
    CallFailure::Error(ErrorV1 {
        kind,
        status: ERROR_STATUS_PERMANENT,
        kind_name: name.to_owned(),
        message,
    })
}

fn invalid_argument(message: impl Into<String>) -> CallFailure {
    binding_error(ERROR_INVALID_ARGUMENT, "InvalidArgument", message)
}

fn buffer_too_large(message: impl Into<String>) -> CallFailure {
    binding_error(ERROR_BUFFER_TOO_LARGE, "BufferTooLarge", message)
}

fn opendal_error_snapshot(error: opendal::Error) -> ErrorV1 {
    let kind = error.kind();
    let code = match kind {
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
    let mut message = error.message().to_owned();
    if u64::try_from(message.len()).map_or(true, |len| len > MAX_OUTPUT_BYTES) {
        message = "OpenDAL error message exceeds the binding output limit".to_owned();
    }
    ErrorV1 {
        kind: code,
        status,
        kind_name: kind.into_static().to_owned(),
        message,
    }
}

fn opendal_error(error: opendal::Error) -> CallFailure {
    CallFailure::Error(opendal_error_snapshot(error))
}

fn construction_error(error: opendal::Error) -> CallFailure {
    let mut snapshot = opendal_error_snapshot(error);
    // Constructor diagnostics can contain backend configuration values. Keep
    // the stable category/status while returning a deliberately generic text.
    snapshot.message = "operator construction failed".to_owned();
    CallFailure::Error(snapshot)
}

fn catch_status(f: impl FnOnce() -> Status) -> Status {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => STATUS_PANIC,
    }
}

fn runtime() -> CallResult<&'static Runtime> {
    let result = RUNTIME.get_or_init(|| {
        opendal::install_default();
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|_| "Tokio runtime initialization failed".to_owned())
    });
    match result {
        Ok(runtime) => Ok(runtime),
        Err(message) => Err(binding_error(
            ERROR_UNEXPECTED,
            "Unexpected",
            message.clone(),
        )),
    }
}

fn is_aligned<T>(pointer: *const T) -> bool {
    pointer.addr().is_multiple_of(align_of::<T>())
}

fn checked_len(length: u64) -> CallResult<usize> {
    let length = usize::try_from(length).map_err(|_| CallFailure::AbiMismatch)?;
    if length > isize::MAX as usize {
        return abi_mismatch();
    }
    Ok(length)
}

fn checked_region(pointer: *const u8, length: usize) -> CallResult<()> {
    if length == 0 {
        return Ok(());
    }
    if pointer.is_null() || pointer.addr().checked_add(length).is_none() {
        return abi_mismatch();
    }
    Ok(())
}

unsafe fn read_required<T: Copy>(pointer: *const T) -> CallResult<T> {
    if pointer.is_null() || !is_aligned(pointer) {
        return abi_mismatch();
    }
    // SAFETY: the ABI contract makes a validated non-null, aligned pointer
    // readable for one complete T. We intentionally copy rather than retain it.
    Ok(unsafe { pointer.read() })
}

unsafe fn borrow_required<'a, T>(pointer: *const T) -> CallResult<&'a T> {
    if pointer.is_null() || !is_aligned(pointer) {
        return abi_mismatch();
    }
    // SAFETY: liveness and type correctness of non-null opaque handles are the
    // caller's documented responsibility; free may not race this borrow.
    Ok(unsafe { &*pointer })
}

unsafe fn borrowed_bytes<'a>(view: BytesViewV1) -> CallResult<&'a [u8]> {
    let length = checked_len(view.len)?;
    if length == 0 {
        return Ok(&[]);
    }
    checked_region(view.data, length)?;
    // SAFETY: checked length/null/address overflow plus the ABI's readable,
    // single-allocation lifetime guarantee satisfy from_raw_parts.
    Ok(unsafe { slice::from_raw_parts(view.data, length) })
}

unsafe fn read_text_value(view: BytesViewV1, label: &str) -> CallResult<String> {
    // SAFETY: delegated to borrowed_bytes; the result is copied immediately.
    let bytes = unsafe { borrowed_bytes(view)? };
    str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| invalid_argument(format!("{label} must be valid UTF-8")))
}

unsafe fn read_text(pointer: *const BytesViewV1, label: &str) -> CallResult<String> {
    // SAFETY: read_required validates the by-value carrier pointer.
    let view = unsafe { read_required(pointer)? };
    // SAFETY: the copied carrier retains the caller-borrowed byte pointer.
    unsafe { read_text_value(view, label) }
}

unsafe fn read_binary(pointer: *const BytesViewV1) -> CallResult<Vec<u8>> {
    // SAFETY: read_required validates the carrier pointer.
    let view = unsafe { read_required(pointer)? };
    // SAFETY: borrowed_bytes validates the byte region; it is copied now.
    Ok(unsafe { borrowed_bytes(view)? }.to_vec())
}

fn absent_bytes_is_canonical(view: BytesViewV1) -> bool {
    view.data.is_null() && view.len == 0
}

unsafe fn optional_text(
    view: BytesViewV1,
    present_bits: u64,
    bit: u64,
    label: &str,
) -> CallResult<Option<String>> {
    if present_bits & bit != 0 {
        // SAFETY: the option struct containing this copied carrier is valid.
        return unsafe { read_text_value(view, label) }.map(Some);
    }
    if !absent_bytes_is_canonical(view) {
        return abi_mismatch();
    }
    Ok(None)
}

unsafe fn read_input_struct<T: Copy>(pointer: *const T) -> CallResult<T> {
    if pointer.is_null() || !is_aligned(pointer) {
        return abi_mismatch();
    }
    // SAFETY: every extensible input begins with this header and caller storage
    // covers it before the declared complete prefix.
    let header = unsafe { pointer.cast::<StructHeaderV1>().read() };
    if header.struct_version != STRUCT_VERSION
        || usize::try_from(header.struct_size).map_or(true, |size| size < size_of::<T>())
    {
        return abi_mismatch();
    }
    // SAFETY: the validated declared size covers this complete v1 prefix.
    Ok(unsafe { pointer.read() })
}

unsafe fn parse_range(range: ByteRangeV1) -> CallResult<BytesRange> {
    if range.struct_version != STRUCT_VERSION
        || usize::try_from(range.struct_size).map_or(true, |size| size < size_of::<ByteRangeV1>())
        || range.reserved0 != 0
    {
        return abi_mismatch();
    }
    match range.kind {
        RANGE_FULL if range.offset == 0 && range.length == 0 => Ok(BytesRange::default()),
        RANGE_FROM if range.length == 0 => Ok(BytesRange::new(range.offset, None)),
        RANGE_OFFSET_LENGTH => {
            if range.offset.checked_add(range.length).is_none() {
                return abi_mismatch();
            }
            Ok(BytesRange::new(range.offset, Some(range.length)))
        }
        RANGE_SUFFIX if range.offset == 0 => Ok(BytesRange::suffix(range.length)),
        _ => abi_mismatch(),
    }
}

unsafe fn parse_read_options(pointer: *const ReadOptionsV1) -> CallResult<ReadOptions> {
    if pointer.is_null() {
        return Ok(ReadOptions::default());
    }
    // SAFETY: non-null options are validated and copied.
    let input = unsafe { read_input_struct(pointer)? };
    let known = READ_VERSION_PRESENT | READ_IF_MATCH_PRESENT | READ_IF_NONE_MATCH_PRESENT;
    if input.present_bits & !known != 0 {
        return abi_mismatch();
    }
    Ok(ReadOptions {
        // SAFETY: the embedded frozen range is validated by parse_range.
        range: unsafe { parse_range(input.range)? },
        // SAFETY: all copied views borrow from the caller for this call.
        version: unsafe {
            optional_text(
                input.version,
                input.present_bits,
                READ_VERSION_PRESENT,
                "read version",
            )?
        },
        // SAFETY: same as above.
        if_match: unsafe {
            optional_text(
                input.if_match,
                input.present_bits,
                READ_IF_MATCH_PRESENT,
                "read if_match",
            )?
        },
        // SAFETY: same as above.
        if_none_match: unsafe {
            optional_text(
                input.if_none_match,
                input.present_bits,
                READ_IF_NONE_MATCH_PRESENT,
                "read if_none_match",
            )?
        },
        ..ReadOptions::default()
    })
}

unsafe fn parse_reader_options(pointer: *const ReaderOptionsV1) -> CallResult<ReaderOptions> {
    if pointer.is_null() {
        return Ok(ReaderOptions::default());
    }
    // SAFETY: non-null options are validated and copied.
    let input = unsafe { read_input_struct(pointer)? };
    let known = READER_VERSION_PRESENT | READER_IF_MATCH_PRESENT | READER_IF_NONE_MATCH_PRESENT;
    if input.present_bits & !known != 0 {
        return abi_mismatch();
    }
    Ok(ReaderOptions {
        // SAFETY: all copied views borrow from the caller for this call.
        version: unsafe {
            optional_text(
                input.version,
                input.present_bits,
                READER_VERSION_PRESENT,
                "reader version",
            )?
        },
        // SAFETY: same as above.
        if_match: unsafe {
            optional_text(
                input.if_match,
                input.present_bits,
                READER_IF_MATCH_PRESENT,
                "reader if_match",
            )?
        },
        // SAFETY: same as above.
        if_none_match: unsafe {
            optional_text(
                input.if_none_match,
                input.present_bits,
                READER_IF_NONE_MATCH_PRESENT,
                "reader if_none_match",
            )?
        },
        ..ReaderOptions::default()
    })
}

struct ParsedReadStreamOptions {
    reader: ReaderOptions,
    range: BytesRange,
    chunk_size: u64,
}

unsafe fn parse_read_stream_options(
    pointer: *const ReadStreamOptionsV1,
) -> CallResult<ParsedReadStreamOptions> {
    if pointer.is_null() {
        return abi_mismatch();
    }
    // SAFETY: read stream options are required, validated, and copied.
    let input = unsafe { read_input_struct(pointer)? };
    let known = READ_STREAM_VERSION_PRESENT
        | READ_STREAM_IF_MATCH_PRESENT
        | READ_STREAM_IF_NONE_MATCH_PRESENT;
    if input.present_bits & !known != 0 {
        return abi_mismatch();
    }
    if input.chunk_size == 0 || input.chunk_size > MAX_OUTPUT_BYTES {
        return Err(invalid_argument(format!(
            "read stream chunk_size must be between 1 and {MAX_OUTPUT_BYTES}"
        )));
    }
    let chunk_size = checked_len(input.chunk_size).map_err(|_| {
        invalid_argument("read stream chunk_size is not representable on this platform")
    })?;
    Ok(ParsedReadStreamOptions {
        // SAFETY: the embedded frozen range is validated by parse_range.
        range: unsafe { parse_range(input.range)? },
        chunk_size: input.chunk_size,
        reader: ReaderOptions {
            // SAFETY: all copied views borrow from the caller only for this call.
            version: unsafe {
                optional_text(
                    input.version,
                    input.present_bits,
                    READ_STREAM_VERSION_PRESENT,
                    "read stream version",
                )?
            },
            // SAFETY: same as above.
            if_match: unsafe {
                optional_text(
                    input.if_match,
                    input.present_bits,
                    READ_STREAM_IF_MATCH_PRESENT,
                    "read stream if_match",
                )?
            },
            // SAFETY: same as above.
            if_none_match: unsafe {
                optional_text(
                    input.if_none_match,
                    input.present_bits,
                    READ_STREAM_IF_NONE_MATCH_PRESENT,
                    "read stream if_none_match",
                )?
            },
            concurrent: 1,
            chunk: Some(chunk_size),
            prefetch: 0,
            ..ReaderOptions::default()
        },
    })
}

unsafe fn parse_stat_options(pointer: *const StatOptionsV1) -> CallResult<StatOptions> {
    if pointer.is_null() {
        return Ok(StatOptions::default());
    }
    // SAFETY: non-null options are validated and copied.
    let input = unsafe { read_input_struct(pointer)? };
    let known = STAT_VERSION_PRESENT | STAT_IF_MATCH_PRESENT | STAT_IF_NONE_MATCH_PRESENT;
    if input.present_bits & !known != 0 {
        return abi_mismatch();
    }
    Ok(StatOptions {
        // SAFETY: copied carriers borrow only for this call.
        version: unsafe {
            optional_text(
                input.version,
                input.present_bits,
                STAT_VERSION_PRESENT,
                "stat version",
            )?
        },
        // SAFETY: same as above.
        if_match: unsafe {
            optional_text(
                input.if_match,
                input.present_bits,
                STAT_IF_MATCH_PRESENT,
                "stat if_match",
            )?
        },
        // SAFETY: same as above.
        if_none_match: unsafe {
            optional_text(
                input.if_none_match,
                input.present_bits,
                STAT_IF_NONE_MATCH_PRESENT,
                "stat if_none_match",
            )?
        },
        ..StatOptions::default()
    })
}

unsafe fn parse_write_options(pointer: *const WriteOptionsV1) -> CallResult<WriteOptions> {
    if pointer.is_null() {
        return Ok(WriteOptions::default());
    }
    // SAFETY: non-null options are validated and copied.
    let input = unsafe { read_input_struct(pointer)? };
    let known_present = WRITE_CONTENT_TYPE_PRESENT
        | WRITE_CONTENT_DISPOSITION_PRESENT
        | WRITE_CONTENT_ENCODING_PRESENT
        | WRITE_CACHE_CONTROL_PRESENT
        | WRITE_IF_MATCH_PRESENT
        | WRITE_IF_NONE_MATCH_PRESENT;
    if input.present_bits & !known_present != 0 || input.flags & !WRITE_APPEND != 0 {
        return abi_mismatch();
    }
    Ok(WriteOptions {
        append: input.flags & WRITE_APPEND != 0,
        // SAFETY: copied carriers borrow only for this call.
        content_type: unsafe {
            optional_text(
                input.content_type,
                input.present_bits,
                WRITE_CONTENT_TYPE_PRESENT,
                "write content_type",
            )?
        },
        // SAFETY: same as above.
        content_disposition: unsafe {
            optional_text(
                input.content_disposition,
                input.present_bits,
                WRITE_CONTENT_DISPOSITION_PRESENT,
                "write content_disposition",
            )?
        },
        // SAFETY: same as above.
        content_encoding: unsafe {
            optional_text(
                input.content_encoding,
                input.present_bits,
                WRITE_CONTENT_ENCODING_PRESENT,
                "write content_encoding",
            )?
        },
        // SAFETY: same as above.
        cache_control: unsafe {
            optional_text(
                input.cache_control,
                input.present_bits,
                WRITE_CACHE_CONTROL_PRESENT,
                "write cache_control",
            )?
        },
        // SAFETY: same as above.
        if_match: unsafe {
            optional_text(
                input.if_match,
                input.present_bits,
                WRITE_IF_MATCH_PRESENT,
                "write if_match",
            )?
        },
        // SAFETY: same as above.
        if_none_match: unsafe {
            optional_text(
                input.if_none_match,
                input.present_bits,
                WRITE_IF_NONE_MATCH_PRESENT,
                "write if_none_match",
            )?
        },
        ..WriteOptions::default()
    })
}

unsafe fn parse_list_options(pointer: *const ListOptionsV1) -> CallResult<ListOptions> {
    if pointer.is_null() {
        return Ok(ListOptions::default());
    }
    // SAFETY: non-null options are validated and copied.
    let input = unsafe { read_input_struct(pointer)? };
    let known_present = LIST_LIMIT_PRESENT | LIST_START_AFTER_PRESENT;
    if input.present_bits & !known_present != 0 || input.flags & !LIST_RECURSIVE != 0 {
        return abi_mismatch();
    }
    let limit = if input.present_bits & LIST_LIMIT_PRESENT != 0 {
        Some(checked_len(input.limit)?)
    } else {
        if input.limit != 0 {
            return abi_mismatch();
        }
        None
    };
    Ok(ListOptions {
        limit,
        // SAFETY: the copied carrier borrows only for this call and is copied.
        start_after: unsafe {
            optional_text(
                input.start_after,
                input.present_bits,
                LIST_START_AFTER_PRESENT,
                "list start_after",
            )?
        },
        recursive: input.flags & LIST_RECURSIVE != 0,
        ..ListOptions::default()
    })
}

unsafe fn parse_delete_options(pointer: *const DeleteOptionsV1) -> CallResult<DeleteOptions> {
    if pointer.is_null() {
        return Ok(DeleteOptions::default());
    }
    // SAFETY: non-null options are validated and copied.
    let input = unsafe { read_input_struct(pointer)? };
    if input.present_bits & !DELETE_VERSION_PRESENT != 0 || input.flags & !DELETE_RECURSIVE != 0 {
        return abi_mismatch();
    }
    Ok(DeleteOptions {
        // SAFETY: copied carrier borrows only for this call.
        version: unsafe {
            optional_text(
                input.version,
                input.present_bits,
                DELETE_VERSION_PRESENT,
                "delete version",
            )?
        },
        recursive: input.flags & DELETE_RECURSIVE != 0,
    })
}

#[cfg(feature = "profile-standard")]
enum ParsedS3CredentialSource {
    DefaultChain {
        disable_ec2_metadata: bool,
    },
    Static {
        access_key_id: String,
        secret_access_key: String,
        session_token: Option<String>,
    },
}

#[cfg(feature = "profile-standard")]
enum ParsedS3Auth {
    Credentials(ParsedS3CredentialSource),
    Unsigned,
    AssumeRole {
        role_arn: String,
        source: ParsedS3CredentialSource,
        external_id: Option<String>,
        role_session_name: Option<String>,
        duration_seconds: Option<u32>,
    },
}

#[cfg(feature = "profile-standard")]
struct ParsedS3Options {
    bucket: String,
    region: String,
    root: Option<String>,
    endpoint: Option<String>,
    virtual_host_style: bool,
    auth: ParsedS3Auth,
}

#[cfg(feature = "profile-standard")]
unsafe fn required_nonempty_text(view: BytesViewV1, label: &str) -> CallResult<String> {
    // SAFETY: the copied carrier borrows only for this call and is copied now.
    let value = unsafe { read_text_value(view, label)? };
    if value.is_empty() {
        return Err(invalid_argument(format!("{label} must not be empty")));
    }
    Ok(value)
}

#[cfg(feature = "profile-standard")]
unsafe fn optional_nonempty_text(
    view: BytesViewV1,
    present_bits: u64,
    bit: u64,
    label: &str,
) -> CallResult<Option<String>> {
    // SAFETY: optional_text validates/copies present views and enforces the
    // canonical absent representation.
    match unsafe { optional_text(view, present_bits, bit, label)? } {
        Some(value) if value.is_empty() => {
            Err(invalid_argument(format!("{label} must not be empty")))
        }
        value => Ok(value),
    }
}

#[cfg(feature = "profile-standard")]
fn require_absent_view(view: BytesViewV1) -> CallResult<()> {
    if absent_bytes_is_canonical(view) {
        Ok(())
    } else {
        abi_mismatch()
    }
}

#[cfg(feature = "profile-standard")]
fn reject_present_field(present_bits: u64, bit: u64, label: &str) -> CallResult<()> {
    if present_bits & bit != 0 {
        Err(invalid_argument(format!(
            "{label} is not valid for the selected S3 authentication mode"
        )))
    } else {
        Ok(())
    }
}

#[cfg(feature = "profile-standard")]
fn reject_role_fields(input: &S3OptionsV1) -> CallResult<()> {
    reject_present_field(input.present_bits, S3_EXTERNAL_ID_PRESENT, "S3 external_id")?;
    reject_present_field(
        input.present_bits,
        S3_ROLE_SESSION_NAME_PRESENT,
        "S3 role_session_name",
    )?;
    reject_present_field(
        input.present_bits,
        S3_ASSUME_ROLE_DURATION_PRESENT,
        "S3 assume-role duration",
    )?;
    require_absent_view(input.role_arn)?;
    require_absent_view(input.external_id)?;
    require_absent_view(input.role_session_name)?;
    if input.assume_role_duration_seconds != 0 {
        return abi_mismatch();
    }
    Ok(())
}

#[cfg(feature = "profile-standard")]
fn reject_static_credentials(input: &S3OptionsV1) -> CallResult<()> {
    reject_present_field(
        input.present_bits,
        S3_SESSION_TOKEN_PRESENT,
        "S3 session_token",
    )?;
    require_absent_view(input.access_key_id)?;
    require_absent_view(input.secret_access_key)?;
    require_absent_view(input.session_token)
}

#[cfg(feature = "profile-standard")]
unsafe fn parse_static_credentials(input: &S3OptionsV1) -> CallResult<ParsedS3CredentialSource> {
    // SAFETY: all copied views borrow only for this call and are copied now.
    let access_key_id = unsafe { required_nonempty_text(input.access_key_id, "S3 access_key_id")? };
    // SAFETY: same as above.
    let secret_access_key =
        unsafe { required_nonempty_text(input.secret_access_key, "S3 secret_access_key")? };
    // SAFETY: same as above.
    let session_token = unsafe {
        optional_nonempty_text(
            input.session_token,
            input.present_bits,
            S3_SESSION_TOKEN_PRESENT,
            "S3 session_token",
        )?
    };
    Ok(ParsedS3CredentialSource::Static {
        access_key_id,
        secret_access_key,
        session_token,
    })
}

#[cfg(feature = "profile-standard")]
unsafe fn parse_s3_options(pointer: *const S3OptionsV1) -> CallResult<ParsedS3Options> {
    if pointer.is_null() {
        return abi_mismatch();
    }
    // SAFETY: the required input struct is validated and copied.
    let input = unsafe { read_input_struct(pointer)? };
    let known_present = S3_ROOT_PRESENT
        | S3_ENDPOINT_PRESENT
        | S3_SESSION_TOKEN_PRESENT
        | S3_EXTERNAL_ID_PRESENT
        | S3_ROLE_SESSION_NAME_PRESENT
        | S3_ASSUME_ROLE_DURATION_PRESENT;
    let known_flags = S3_VIRTUAL_HOST_STYLE | S3_DISABLE_EC2_METADATA;
    if input.present_bits & !known_present != 0
        || input.flags & !known_flags != 0
        || input.reserved0 != 0
    {
        return abi_mismatch();
    }

    // SAFETY: required and optional text views borrow only for this call and
    // are copied before construction proceeds.
    let bucket = unsafe { required_nonempty_text(input.bucket, "S3 bucket")? };
    // SAFETY: same as above.
    let region = unsafe { required_nonempty_text(input.region, "S3 region")? };
    // SAFETY: same as above.
    let root = unsafe {
        optional_nonempty_text(input.root, input.present_bits, S3_ROOT_PRESENT, "S3 root")?
    };
    // SAFETY: same as above.
    let endpoint = unsafe {
        optional_nonempty_text(
            input.endpoint,
            input.present_bits,
            S3_ENDPOINT_PRESENT,
            "S3 endpoint",
        )?
    };

    let disable_ec2_metadata = input.flags & S3_DISABLE_EC2_METADATA != 0;
    let auth = match input.auth_kind {
        S3_AUTH_DEFAULT_CHAIN => {
            if input.source_kind != S3_SOURCE_DEFAULT_CHAIN {
                return Err(invalid_argument(
                    "default-chain S3 authentication requires the default source kind",
                ));
            }
            reject_static_credentials(&input)?;
            reject_role_fields(&input)?;
            ParsedS3Auth::Credentials(ParsedS3CredentialSource::DefaultChain {
                disable_ec2_metadata,
            })
        }
        S3_AUTH_STATIC => {
            if input.source_kind != S3_SOURCE_STATIC {
                return Err(invalid_argument(
                    "static S3 authentication requires the static source kind",
                ));
            }
            if disable_ec2_metadata {
                return Err(invalid_argument(
                    "disable_ec2_metadata is only valid for a default credential chain",
                ));
            }
            reject_role_fields(&input)?;
            // SAFETY: the static mode makes both key views required and permits
            // only the optional session token.
            ParsedS3Auth::Credentials(unsafe { parse_static_credentials(&input)? })
        }
        S3_AUTH_UNSIGNED => {
            if input.source_kind != S3_SOURCE_DEFAULT_CHAIN {
                return Err(invalid_argument(
                    "unsigned S3 authentication requires the default source kind",
                ));
            }
            if disable_ec2_metadata {
                return Err(invalid_argument(
                    "disable_ec2_metadata is not valid for unsigned S3 authentication",
                ));
            }
            reject_static_credentials(&input)?;
            reject_role_fields(&input)?;
            ParsedS3Auth::Unsigned
        }
        S3_AUTH_ASSUME_ROLE => {
            // SAFETY: role ARN is required in assume-role mode and copied now.
            let role_arn = unsafe { required_nonempty_text(input.role_arn, "S3 role_arn")? };
            // SAFETY: optional role values borrow only for this call and are copied now.
            let external_id = unsafe {
                optional_nonempty_text(
                    input.external_id,
                    input.present_bits,
                    S3_EXTERNAL_ID_PRESENT,
                    "S3 external_id",
                )?
            };
            // SAFETY: same as above.
            let role_session_name = unsafe {
                optional_nonempty_text(
                    input.role_session_name,
                    input.present_bits,
                    S3_ROLE_SESSION_NAME_PRESENT,
                    "S3 role_session_name",
                )?
            };
            let duration_seconds = if input.present_bits & S3_ASSUME_ROLE_DURATION_PRESENT != 0 {
                if input.assume_role_duration_seconds == 0 {
                    return Err(invalid_argument(
                        "S3 assume-role duration_seconds must be positive",
                    ));
                }
                Some(input.assume_role_duration_seconds)
            } else {
                if input.assume_role_duration_seconds != 0 {
                    return abi_mismatch();
                }
                None
            };
            let source = match input.source_kind {
                S3_SOURCE_DEFAULT_CHAIN => {
                    reject_static_credentials(&input)?;
                    ParsedS3CredentialSource::DefaultChain {
                        disable_ec2_metadata,
                    }
                }
                S3_SOURCE_STATIC => {
                    if disable_ec2_metadata {
                        return Err(invalid_argument(
                            "disable_ec2_metadata is only valid for a default credential chain",
                        ));
                    }
                    // SAFETY: a static assume-role source requires/copies both
                    // keys and permits the optional session token.
                    unsafe { parse_static_credentials(&input)? }
                }
                _ => {
                    return Err(invalid_argument(
                        "unsupported S3 assume-role credential source kind",
                    ));
                }
            };
            ParsedS3Auth::AssumeRole {
                role_arn,
                source,
                external_id,
                role_session_name,
                duration_seconds,
            }
        }
        _ => return Err(invalid_argument("unsupported S3 authentication kind")),
    };

    Ok(ParsedS3Options {
        bucket,
        region,
        root,
        endpoint,
        virtual_host_style: input.flags & S3_VIRTUAL_HOST_STYLE != 0,
        auth,
    })
}

unsafe fn clear_required_output<T>(output: *mut T, value: T) -> CallResult<()> {
    if output.is_null() || !is_aligned(output) {
        return abi_mismatch();
    }
    // SAFETY: caller provides writable, aligned storage for one T.
    unsafe { output.write(value) };
    Ok(())
}

unsafe fn clear_error_output(output: *mut *mut ErrorV1) -> CallResult<()> {
    if output.is_null() {
        return Ok(());
    }
    if !is_aligned(output) {
        return abi_mismatch();
    }
    // SAFETY: non-null out_error is writable for one pointer.
    unsafe { output.write(ptr::null_mut()) };
    Ok(())
}

fn combine_output_validation<const N: usize>(results: [CallResult<()>; N]) -> CallResult<()> {
    // Array elements are evaluated before this function is entered, so every
    // output slot gets an independent clear attempt even if an earlier slot is
    // invalid. Return the first validation failure only after all attempts.
    for result in results {
        result?;
    }
    Ok(())
}

unsafe fn finish_failure(failure: CallFailure, out_error: *mut *mut ErrorV1) -> Status {
    match failure {
        CallFailure::AbiMismatch => STATUS_ABI_MISMATCH,
        CallFailure::Error(error) => {
            if !out_error.is_null() {
                let error = Box::into_raw(Box::new(error));
                // SAFETY: out_error was validated and cleared before work.
                unsafe { out_error.write(error) };
            }
            STATUS_ERROR
        }
    }
}

unsafe fn prepare_view<T>(output: *mut T) -> CallResult<StructHeaderV1> {
    if output.is_null() || !is_aligned(output) {
        return abi_mismatch();
    }
    // SAFETY: all output views start with this readable header.
    let header = unsafe { output.cast::<StructHeaderV1>().read() };
    if header.struct_version != STRUCT_VERSION
        || usize::try_from(header.struct_size).map_or(true, |size| size < size_of::<T>())
    {
        return abi_mismatch();
    }
    // SAFETY: the declared writable size covers exactly this known v1 view.
    unsafe { ptr::write_bytes(output.cast::<u8>(), 0, size_of::<T>()) };
    // SAFETY: restore the caller-owned header after clearing the payload.
    unsafe { output.cast::<StructHeaderV1>().write(header) };
    Ok(header)
}

fn bytes_view(bytes: &[u8]) -> BytesViewV1 {
    if bytes.is_empty() {
        BytesViewV1 {
            data: ptr::null(),
            len: 0,
        }
    } else {
        BytesViewV1 {
            data: bytes.as_ptr(),
            len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        }
    }
}

fn string_view(value: &str) -> BytesViewV1 {
    bytes_view(value.as_bytes())
}

fn optional_string_view(value: Option<&str>) -> BytesViewV1 {
    value.map_or(
        BytesViewV1 {
            data: ptr::null(),
            len: 0,
        },
        string_view,
    )
}

fn capability_view(capability: Capability) -> CapabilityV1 {
    let mut word0 = 0;
    if capability.stat {
        word0 |= CAP_STAT;
    }
    if capability.read {
        word0 |= CAP_READ;
    }
    if capability.write {
        word0 |= CAP_WRITE;
    }
    if capability.create_dir {
        word0 |= CAP_CREATE_DIR;
    }
    if capability.delete {
        word0 |= CAP_DELETE;
    }
    if capability.list {
        word0 |= CAP_LIST;
    }
    if capability.copy {
        word0 |= CAP_COPY;
    }
    if capability.rename {
        word0 |= CAP_RENAME;
    }
    if capability.read_with_suffix {
        word0 |= CAP_READ_SUFFIX;
    }
    if capability.write_can_append {
        word0 |= CAP_WRITE_APPEND;
    }
    if capability.list_with_limit {
        word0 |= CAP_LIST_LIMIT;
    }
    if capability.list_with_start_after {
        word0 |= CAP_LIST_START_AFTER;
    }
    if capability.list_with_recursive {
        word0 |= CAP_LIST_RECURSIVE;
    }
    if capability.presign_stat {
        word0 |= CAP_PRESIGN_STAT;
    }
    if capability.presign_read {
        word0 |= CAP_PRESIGN_READ;
    }
    if capability.presign_write {
        word0 |= CAP_PRESIGN_WRITE;
    }
    CapabilityV1 {
        words: [word0, 0, 0, 0],
    }
}

fn checked_presigned_request(request: PresignedRequest) -> CallResult<PresignedRequestV1> {
    let method = request.method().as_str().to_owned();
    let uri = request.uri().to_string();
    let mut total_len = method
        .len()
        .checked_add(uri.len())
        .ok_or_else(|| buffer_too_large("presigned request exceeds the binding output limit"))?;
    let mut headers = Vec::with_capacity(request.header().len());
    for (name, value) in request.header() {
        let name = name.as_str().to_owned();
        let value = value.as_bytes().to_vec();
        total_len = total_len
            .checked_add(name.len())
            .and_then(|length| length.checked_add(value.len()))
            .ok_or_else(|| {
                buffer_too_large("presigned request exceeds the binding output limit")
            })?;
        headers.push(PresignedHeaderV1 { name, value });
    }
    if u64::try_from(total_len).map_or(true, |length| length > MAX_OUTPUT_BYTES)
        || u64::try_from(headers.len()).map_or(true, |length| length > MAX_OUTPUT_BYTES)
    {
        return Err(buffer_too_large(
            "presigned request exceeds the binding output limit",
        ));
    }
    Ok(PresignedRequestV1 {
        method,
        uri,
        headers,
    })
}

fn presign_expiry(seconds: u64) -> CallResult<Duration> {
    if seconds == 0 {
        return Err(invalid_argument(
            "presign expires_in_seconds must be greater than zero",
        ));
    }
    Ok(Duration::from_secs(seconds))
}

fn check_output_string(value: Option<&str>) -> CallResult<()> {
    if value.is_some_and(|value| {
        u64::try_from(value.len()).map_or(true, |length| length > MAX_OUTPUT_BYTES)
    }) {
        return Err(buffer_too_large(
            "metadata string exceeds the binding output limit",
        ));
    }
    Ok(())
}

fn checked_metadata(metadata: Metadata) -> CallResult<MetadataV1> {
    check_output_string(metadata.cache_control())?;
    check_output_string(metadata.content_disposition())?;
    check_output_string(metadata.content_encoding())?;
    check_output_string(metadata.content_md5())?;
    check_output_string(metadata.content_type())?;
    check_output_string(metadata.etag())?;
    check_output_string(metadata.version())?;
    Ok(MetadataV1 { metadata })
}

fn checked_entry(entry: opendal::Entry) -> CallResult<EntryV1> {
    let name = entry.name().to_owned();
    let (path, metadata) = entry.into_parts();
    if u64::try_from(path.len()).map_or(true, |length| length > MAX_OUTPUT_BYTES) {
        return Err(buffer_too_large(
            "entry path exceeds the binding output limit",
        ));
    }
    if u64::try_from(name.len()).map_or(true, |length| length > MAX_OUTPUT_BYTES) {
        return Err(buffer_too_large(
            "entry name exceeds the binding output limit",
        ));
    }
    let metadata = checked_metadata(metadata)?.metadata;
    Ok(EntryV1 {
        path,
        name,
        metadata,
    })
}

fn lock_lister_state(lister: &ListerV1) -> std::sync::MutexGuard<'_, ListerStateV1> {
    match lister.state.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn drop_blocking_lister_contained(lister: opendal::blocking::Lister) {
    let _ = catch_unwind(AssertUnwindSafe(|| drop(lister)));
}

fn close_lister_state(lister: &ListerV1) {
    let open = {
        let mut state = lock_lister_state(lister);
        let previous = std::mem::replace(&mut *state, ListerStateV1::Closed);
        lister.state.clear_poison();
        match previous {
            ListerStateV1::Open(inner) => Some(inner),
            ListerStateV1::Exhausted | ListerStateV1::Closed => None,
        }
    };
    if let Some(inner) = open {
        drop_blocking_lister_contained(inner);
    }
}

fn drop_blocking_reader_contained(reader: opendal::blocking::Reader) {
    let _ = catch_unwind(AssertUnwindSafe(|| drop(reader)));
}

fn close_reader_state(reader: &ReaderV1) {
    let open = {
        let mut state = match reader.state.write() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        let previous = std::mem::replace(&mut *state, ReaderStateV1::Closed);
        reader.state.clear_poison();
        match previous {
            ReaderStateV1::Open(inner) => Some(inner),
            ReaderStateV1::Closed => None,
        }
    };
    if let Some(inner) = open {
        drop_blocking_reader_contained(inner);
    }
}

fn close_read_stream_state(stream: &ReadStreamV1) {
    let mut state = match stream.state.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    };
    *state = ReadStreamStateV1::Closed;
    stream.state.clear_poison();
}

fn fail_read_stream_state(stream: &ReadStreamV1) {
    let mut state = match stream.state.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    };
    *state = ReadStreamStateV1::Failed;
    stream.state.clear_poison();
}

fn read_stream_lock_failure(stream: &ReadStreamV1) -> CallFailure {
    fail_read_stream_state(stream);
    binding_error(
        ERROR_UNEXPECTED,
        "Unexpected",
        "read stream state lock was poisoned",
    )
}

fn drop_async_writer_contained(writer: opendal::Writer) {
    let _ = catch_unwind(AssertUnwindSafe(|| drop(writer)));
}

fn fail_writer_state(writer: &WriterV1) {
    let open = {
        let mut state = match writer.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        let previous = std::mem::replace(&mut *state, WriterStateV1::Failed);
        writer.state.clear_poison();
        writer.changed.notify_all();
        match previous {
            WriterStateV1::Open(inner) => Some(inner),
            WriterStateV1::Busy
            | WriterStateV1::Finished
            | WriterStateV1::Aborted
            | WriterStateV1::Failed => None,
        }
    };
    if let Some(inner) = open {
        drop_async_writer_contained(inner);
    }
}

fn writer_poison_failure(
    writer: &WriterV1,
    mut state: MutexGuard<'_, WriterStateV1>,
) -> CallFailure {
    let previous = std::mem::replace(&mut *state, WriterStateV1::Failed);
    writer.state.clear_poison();
    writer.changed.notify_all();
    let open = match previous {
        WriterStateV1::Open(inner) => Some(inner),
        WriterStateV1::Busy
        | WriterStateV1::Finished
        | WriterStateV1::Aborted
        | WriterStateV1::Failed => None,
    };
    drop(state);
    if let Some(inner) = open {
        drop_async_writer_contained(inner);
    }
    binding_error(
        ERROR_UNEXPECTED,
        "Unexpected",
        "writer state lock was poisoned",
    )
}

fn take_writer_for_call(
    writer: &WriterV1,
    repeated_abort_is_ok: bool,
) -> CallResult<Option<opendal::Writer>> {
    let mut state = match writer.state.lock() {
        Ok(state) => state,
        Err(poisoned) => return Err(writer_poison_failure(writer, poisoned.into_inner())),
    };
    loop {
        let current = std::mem::replace(&mut *state, WriterStateV1::Busy);
        match current {
            WriterStateV1::Open(inner) => return Ok(Some(inner)),
            WriterStateV1::Busy => {
                *state = WriterStateV1::Busy;
                state = match writer.changed.wait(state) {
                    Ok(state) => state,
                    Err(poisoned) => {
                        return Err(writer_poison_failure(writer, poisoned.into_inner()));
                    }
                };
            }
            WriterStateV1::Aborted if repeated_abort_is_ok => {
                *state = WriterStateV1::Aborted;
                return Ok(None);
            }
            terminal @ (WriterStateV1::Finished
            | WriterStateV1::Aborted
            | WriterStateV1::Failed) => {
                *state = terminal;
                return Err(binding_error(
                    ERROR_RESOURCE_CLOSED,
                    "ResourceClosed",
                    "writer is closed",
                ));
            }
        }
    }
}

fn set_writer_state(writer: &WriterV1, next: WriterStateV1) -> CallResult<()> {
    let mut state = match writer.state.lock() {
        Ok(state) => state,
        Err(poisoned) => {
            let mut state = poisoned.into_inner();
            *state = WriterStateV1::Failed;
            writer.state.clear_poison();
            writer.changed.notify_all();
            return Err(binding_error(
                ERROR_UNEXPECTED,
                "Unexpected",
                "writer state lock was poisoned",
            ));
        }
    };
    *state = next;
    writer.changed.notify_all();
    Ok(())
}

fn checked_read_buffer(
    buffer: opendal::Buffer,
    max_output_len: u64,
    subject: &str,
) -> CallResult<Box<BufferV1>> {
    let length = u64::try_from(buffer.len())
        .map_err(|_| buffer_too_large(format!("{subject} length is not representable")))?;
    if length > MAX_OUTPUT_BYTES || length > max_output_len {
        return Err(buffer_too_large(format!(
            "{subject} exceeds the negotiated output limit"
        )));
    }
    Ok(Box::new(BufferV1 {
        bytes: buffer.to_vec(),
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WholeReadRangePlan {
    Bounded(BytesRange),
    Suffix { size: u64 },
    OpenEnded { offset: u64, probe_size: u64 },
}

struct WholeReadPlan {
    reader_options: ReaderOptions,
    stat_options: StatOptions,
    range: WholeReadRangePlan,
    output_limit: u64,
}

fn whole_read_plan(options: ReadOptions, max_output_len: u64) -> CallResult<WholeReadPlan> {
    let output_limit = max_output_len.min(MAX_OUTPUT_BYTES);
    let ReadOptions {
        range,
        version,
        if_match,
        if_none_match,
        if_modified_since,
        if_unmodified_since,
        content_length_hint,
        ..
    } = options;

    let range = match range {
        BytesRange::Range {
            offset,
            size: Some(size),
        } => {
            if size > output_limit {
                return Err(buffer_too_large(
                    "read result exceeds the negotiated output limit",
                ));
            }
            WholeReadRangePlan::Bounded(BytesRange::new(offset, Some(size)))
        }
        BytesRange::Suffix { size } => {
            if size > output_limit {
                return Err(buffer_too_large(
                    "read result exceeds the negotiated output limit",
                ));
            }
            WholeReadRangePlan::Suffix { size }
        }
        BytesRange::Range { offset, size: None } => {
            // Reading one byte beyond the negotiated limit distinguishes an exact
            // fit from an oversized Full/From result without materializing the
            // rest of the object. Clamp at the u64 address-space boundary so a
            // valid open-ended range near u64::MAX cannot overflow when bounded.
            let probe_size = output_limit
                .checked_add(1)
                .expect("MAX_OUTPUT_BYTES leaves room for the overflow probe")
                .min(u64::MAX - offset);
            WholeReadRangePlan::OpenEnded { offset, probe_size }
        }
    };

    Ok(WholeReadPlan {
        stat_options: StatOptions {
            version: version.clone(),
            ..StatOptions::default()
        },
        reader_options: ReaderOptions {
            version,
            if_match,
            if_none_match,
            if_modified_since,
            if_unmodified_since,
            content_length_hint,
            concurrent: 1,
            chunk: Some(WHOLE_READ_CHUNK_BYTES),
            gap: None,
            prefetch: 0,
        },
        range,
        output_limit,
    })
}

fn resolve_whole_read_range(
    operator: &opendal::blocking::Operator,
    path: &str,
    plan: &WholeReadPlan,
) -> CallResult<BytesRange> {
    let (offset, requested_size) = match plan.range {
        WholeReadRangePlan::Bounded(range) => return Ok(range),
        WholeReadRangePlan::Suffix { size } => (None, size),
        WholeReadRangePlan::OpenEnded { offset, probe_size } => (Some(offset), probe_size),
    };
    let metadata = operator
        .stat_options(path, plan.stat_options.clone())
        .map_err(opendal_error)?;
    if metadata.is_dir() {
        return Err(opendal_error(
            opendal::Error::new(ErrorKind::IsADirectory, "read path is a directory")
                .with_operation("Operator::read"),
        ));
    }
    let content_length = metadata.content_length();
    match offset {
        Some(offset) => {
            let start = offset.min(content_length);
            let size = content_length.saturating_sub(start).min(requested_size);
            Ok(BytesRange::new(start, Some(size)))
        }
        None => {
            let size = content_length.min(requested_size);
            Ok(BytesRange::new(content_length - size, Some(size)))
        }
    }
}

fn append_whole_read_chunk(
    output: &mut Vec<u8>,
    buffer: opendal::Buffer,
    output_limit: u64,
) -> CallResult<()> {
    let current_length = u64::try_from(output.len())
        .map_err(|_| buffer_too_large("read result length is not representable"))?;
    let chunk_length = u64::try_from(buffer.len())
        .map_err(|_| buffer_too_large("read result length is not representable"))?;
    let next_length = current_length
        .checked_add(chunk_length)
        .ok_or_else(|| buffer_too_large("read result length is not representable"))?;
    if next_length > output_limit {
        return Err(buffer_too_large(
            "read result exceeds the negotiated output limit",
        ));
    }

    let required = usize::try_from(next_length)
        .map_err(|_| buffer_too_large("read result length is not representable"))?;
    if required > output.capacity() {
        let limit = usize::try_from(output_limit)
            .map_err(|_| buffer_too_large("read output limit is not representable"))?;
        let target = output
            .capacity()
            .max(1)
            .saturating_mul(2)
            .max(required)
            .min(limit);
        output.reserve_exact(target - output.len());
    }
    for bytes in buffer {
        output.extend_from_slice(&bytes);
    }
    debug_assert_eq!(output.len(), required);
    Ok(())
}

fn metadata_output_view(metadata: &Metadata, header: StructHeaderV1) -> CallResult<MetadataViewV1> {
    let mut present_bits = 0;
    let is_current = match metadata.is_current() {
        Some(value) => {
            present_bits |= METADATA_IS_CURRENT_PRESENT;
            u32::from(value)
        }
        None => 0,
    };
    let last_modified = match metadata.last_modified() {
        Some(value) => {
            present_bits |= METADATA_LAST_MODIFIED_PRESENT;
            let value = value.into_inner();
            let nanoseconds =
                u32::try_from(value.subsec_nanosecond()).map_err(|_| CallFailure::AbiMismatch)?;
            TimestampV1 {
                unix_seconds: value.as_second(),
                nanoseconds,
                reserved0: 0,
            }
        }
        None => TimestampV1::default(),
    };
    if metadata.cache_control().is_some() {
        present_bits |= METADATA_CACHE_CONTROL_PRESENT;
    }
    if metadata.content_disposition().is_some() {
        present_bits |= METADATA_CONTENT_DISPOSITION_PRESENT;
    }
    if metadata.content_encoding().is_some() {
        present_bits |= METADATA_CONTENT_ENCODING_PRESENT;
    }
    if metadata.content_md5().is_some() {
        present_bits |= METADATA_CONTENT_MD5_PRESENT;
    }
    if metadata.content_type().is_some() {
        present_bits |= METADATA_CONTENT_TYPE_PRESENT;
    }
    if metadata.etag().is_some() {
        present_bits |= METADATA_ETAG_PRESENT;
    }
    if metadata.version().is_some() {
        present_bits |= METADATA_VERSION_PRESENT;
    }
    let mode = match metadata.mode() {
        EntryMode::FILE => ENTRY_MODE_FILE,
        EntryMode::DIR => ENTRY_MODE_DIRECTORY,
        EntryMode::Unknown => ENTRY_MODE_UNKNOWN,
    };
    Ok(MetadataViewV1 {
        struct_size: header.struct_size,
        struct_version: header.struct_version,
        present_bits,
        mode,
        is_current,
        is_deleted: u32::from(metadata.is_deleted()),
        reserved0: 0,
        content_length: metadata.content_length(),
        last_modified,
        cache_control: optional_string_view(metadata.cache_control()),
        content_disposition: optional_string_view(metadata.content_disposition()),
        content_encoding: optional_string_view(metadata.content_encoding()),
        content_md5: optional_string_view(metadata.content_md5()),
        content_type: optional_string_view(metadata.content_type()),
        etag: optional_string_view(metadata.etag()),
        version: optional_string_view(metadata.version()),
    })
}

unsafe extern "C" fn library_info(output: *mut LibraryInfoViewV1) -> Status {
    catch_status(|| {
        // SAFETY: prepare_view performs all detectable output validation.
        let header = match unsafe { prepare_view(output) } {
            Ok(header) => header,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        let view = LibraryInfoViewV1 {
            struct_size: header.struct_size,
            struct_version: header.struct_version,
            reserved0: 0,
            binding_version: string_view(BINDING_VERSION),
            opendal_version: string_view(OPENDAL_VERSION),
            service_profile: string_view(SERVICE_PROFILE),
        };
        // SAFETY: prepare_view validated writable storage for the complete view.
        unsafe { output.write(view) };
        STATUS_OK
    })
}

unsafe extern "C" fn error_view(error: *const ErrorV1, output: *mut ErrorViewV1) -> Status {
    catch_status(|| {
        // SAFETY: output is validated and cleared before the handle is inspected.
        let header = match unsafe { prepare_view(output) } {
            Ok(header) => header,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let error = match unsafe { borrow_required(error) } {
            Ok(error) => error,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        let view = ErrorViewV1 {
            struct_size: header.struct_size,
            struct_version: header.struct_version,
            kind: error.kind,
            status: error.status,
            kind_name: string_view(&error.kind_name),
            message: string_view(&error.message),
        };
        // SAFETY: prepare_view validated writable storage for the complete view.
        unsafe { output.write(view) };
        STATUS_OK
    })
}

unsafe extern "C" fn error_free(error: *mut ErrorV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !error.is_null() {
            // SAFETY: every non-null handle is uniquely owned and freed once.
            unsafe { drop(Box::from_raw(error)) };
        }
    }));
}

unsafe extern "C" fn buffer_len(buffer: *const BufferV1, output: *mut u64) -> Status {
    catch_status(|| {
        // SAFETY: clear validates the required scalar output.
        if let Err(failure) = unsafe { clear_required_output(output, 0) } {
            return unsafe { finish_failure(failure, ptr::null_mut()) };
        }
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let buffer = match unsafe { borrow_required(buffer) } {
            Ok(buffer) => buffer,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        let length = match u64::try_from(buffer.bytes.len()) {
            Ok(length) => length,
            Err(_) => return STATUS_ABI_MISMATCH,
        };
        // SAFETY: output was validated above.
        unsafe { output.write(length) };
        STATUS_OK
    })
}

unsafe extern "C" fn buffer_copy(
    buffer: *const BufferV1,
    destination: *mut u8,
    capacity: u64,
    out_required: *mut u64,
) -> Status {
    catch_status(|| {
        // SAFETY: clear validates the required scalar output.
        if let Err(failure) = unsafe { clear_required_output(out_required, 0) } {
            return unsafe { finish_failure(failure, ptr::null_mut()) };
        }
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let buffer = match unsafe { borrow_required(buffer) } {
            Ok(buffer) => buffer,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        let required = match u64::try_from(buffer.bytes.len()) {
            Ok(length) => length,
            Err(_) => return STATUS_ABI_MISMATCH,
        };
        if destination.is_null() {
            if capacity != 0 {
                return STATUS_ABI_MISMATCH;
            }
            // SAFETY: out_required was validated above.
            unsafe { out_required.write(required) };
            return if required == 0 {
                STATUS_OK
            } else {
                STATUS_BUFFER_TOO_SMALL
            };
        }
        let capacity_usize = match checked_len(capacity) {
            Ok(capacity) => capacity,
            Err(_) => return STATUS_ABI_MISMATCH,
        };
        if destination.addr().checked_add(capacity_usize).is_none() {
            return STATUS_ABI_MISMATCH;
        }
        if capacity < required {
            // SAFETY: out_required was validated above; destination is untouched.
            unsafe { out_required.write(required) };
            return STATUS_BUFFER_TOO_SMALL;
        }
        // No fallible or panicking work remains before the atomic copy.
        // SAFETY: capacity covers the validated immutable buffer and the caller
        // guarantees writable, non-overlapping storage for that prefix.
        unsafe {
            out_required.write(required);
            if !buffer.bytes.is_empty() {
                ptr::copy_nonoverlapping(buffer.bytes.as_ptr(), destination, buffer.bytes.len());
            }
        }
        STATUS_OK
    })
}

unsafe extern "C" fn buffer_free(buffer: *mut BufferV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !buffer.is_null() {
            // SAFETY: every non-null handle is uniquely owned and freed once.
            unsafe { drop(Box::from_raw(buffer)) };
        }
    }));
}

unsafe extern "C" fn metadata_view(
    metadata: *const MetadataV1,
    output: *mut MetadataViewV1,
) -> Status {
    catch_status(|| {
        // SAFETY: output is validated and cleared first.
        let header = match unsafe { prepare_view(output) } {
            Ok(header) => header,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let metadata = match unsafe { borrow_required(metadata) } {
            Ok(metadata) => metadata,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        let view = match metadata_output_view(&metadata.metadata, header) {
            Ok(view) => view,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        // SAFETY: prepare_view validated complete writable storage.
        unsafe { output.write(view) };
        STATUS_OK
    })
}

unsafe extern "C" fn metadata_free(metadata: *mut MetadataV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !metadata.is_null() {
            // SAFETY: every non-null handle is uniquely owned and freed once.
            unsafe { drop(Box::from_raw(metadata)) };
        }
    }));
}

unsafe extern "C" fn entry_view(entry: *const EntryV1, output: *mut EntryViewV1) -> Status {
    catch_status(|| {
        // SAFETY: output is validated and cleared first.
        let header = match unsafe { prepare_view(output) } {
            Ok(header) => header,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let entry = match unsafe { borrow_required(entry) } {
            Ok(entry) => entry,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        let view = EntryViewV1 {
            struct_size: header.struct_size,
            struct_version: header.struct_version,
            reserved0: 0,
            path: string_view(&entry.path),
            name: string_view(&entry.name),
        };
        // SAFETY: prepare_view validated complete writable storage.
        unsafe { output.write(view) };
        STATUS_OK
    })
}

unsafe extern "C" fn entry_metadata_view(
    entry: *const EntryV1,
    output: *mut MetadataViewV1,
) -> Status {
    catch_status(|| {
        // SAFETY: output is validated and cleared first.
        let header = match unsafe { prepare_view(output) } {
            Ok(header) => header,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let entry = match unsafe { borrow_required(entry) } {
            Ok(entry) => entry,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        let view = match metadata_output_view(&entry.metadata, header) {
            Ok(view) => view,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        // SAFETY: prepare_view validated complete writable storage.
        unsafe { output.write(view) };
        STATUS_OK
    })
}

unsafe extern "C" fn entry_free(entry: *mut EntryV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !entry.is_null() {
            // SAFETY: every non-null handle is uniquely owned and freed once.
            unsafe { drop(Box::from_raw(entry)) };
        }
    }));
}

unsafe extern "C" fn operator_info_view(
    info: *const OperatorInfoV1,
    output: *mut OperatorInfoViewV1,
) -> Status {
    catch_status(|| {
        // SAFETY: output is validated and cleared first.
        let header = match unsafe { prepare_view(output) } {
            Ok(header) => header,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let info = match unsafe { borrow_required(info) } {
            Ok(info) => info,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        let view = OperatorInfoViewV1 {
            struct_size: header.struct_size,
            struct_version: header.struct_version,
            reserved0: 0,
            scheme: string_view(&info.scheme),
            root: string_view(&info.root),
            name: string_view(&info.name),
            capability: info.capability,
        };
        // SAFETY: prepare_view validated complete writable storage.
        unsafe { output.write(view) };
        STATUS_OK
    })
}

unsafe extern "C" fn operator_info_free(info: *mut OperatorInfoV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !info.is_null() {
            // SAFETY: every non-null handle is uniquely owned and freed once.
            unsafe { drop(Box::from_raw(info)) };
        }
    }));
}

unsafe fn read_config(config: *const KvV1, config_len: u64) -> CallResult<Vec<(String, String)>> {
    let length = checked_len(config_len)?;
    let bytes = length
        .checked_mul(size_of::<KvV1>())
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or(CallFailure::AbiMismatch)?;
    if length == 0 {
        return Ok(Vec::new());
    }
    if config.is_null()
        || !is_aligned(config)
        || config.cast::<u8>().addr().checked_add(bytes).is_none()
    {
        return abi_mismatch();
    }
    // SAFETY: length, total bytes, alignment and non-null were checked, and the
    // ABI guarantees one fully initialized contiguous allocation.
    let config = unsafe { slice::from_raw_parts(config, length) };
    let mut output = Vec::with_capacity(length);
    let mut keys = HashSet::with_capacity(length);
    for item in config {
        // SAFETY: nested views are borrowed from the valid config element.
        let key = unsafe { read_text_value(item.key, "config key")? };
        if !keys.insert(key.to_ascii_lowercase()) {
            return Err(invalid_argument("duplicate config key"));
        }
        // SAFETY: nested views are borrowed from the valid config element.
        let value = unsafe { read_text_value(item.value, "config value")? };
        output.push((key, value));
    }
    Ok(output)
}

fn operator_handles_with_layers(
    async_operator: opendal::Operator,
    layer_bits: u32,
) -> CallResult<(Box<OperatorV1>, Box<OperatorInfoV1>)> {
    let operator =
        opendal::blocking::Operator::new(async_operator.clone()).map_err(construction_error)?;
    let info = operator.info();
    let scheme = info.scheme().to_owned();
    let root = info.root();
    let name = info.name();
    for value in [&scheme, &root, &name] {
        if u64::try_from(value.len()).map_or(true, |len| len > MAX_OUTPUT_BYTES) {
            return Err(buffer_too_large(
                "operator information exceeds the binding output limit",
            ));
        }
    }
    let info = OperatorInfoV1 {
        scheme,
        root,
        name,
        capability: capability_view(info.capability()),
    };
    Ok((
        Box::new(OperatorV1 {
            async_inner: async_operator,
            inner: operator,
            layer_bits,
        }),
        Box::new(info),
    ))
}

fn operator_handles(
    async_operator: opendal::Operator,
) -> CallResult<(Box<OperatorV1>, Box<OperatorInfoV1>)> {
    operator_handles_with_layers(async_operator, 0)
}

#[cfg(feature = "profile-standard")]
fn configure_s3_source(
    mut builder: opendal::services::S3,
    source: ParsedS3CredentialSource,
) -> opendal::services::S3 {
    match source {
        ParsedS3CredentialSource::DefaultChain {
            disable_ec2_metadata,
        } => {
            if disable_ec2_metadata {
                builder = builder.disable_ec2_metadata();
            }
            builder
        }
        ParsedS3CredentialSource::Static {
            access_key_id,
            secret_access_key,
            session_token,
        } => {
            builder = builder
                .access_key_id(&access_key_id)
                .secret_access_key(&secret_access_key)
                .disable_config_load()
                .disable_ec2_metadata();
            if let Some(session_token) = session_token {
                builder = builder.session_token(&session_token);
            }
            builder
        }
    }
}

#[cfg(feature = "profile-standard")]
fn build_s3_operator(options: ParsedS3Options) -> CallResult<opendal::Operator> {
    let ParsedS3Options {
        bucket,
        region,
        root,
        endpoint,
        virtual_host_style,
        auth,
    } = options;
    let mut builder = opendal::services::S3::default()
        .bucket(&bucket)
        .region(&region);
    if let Some(root) = root {
        builder = builder.root(&root);
    }
    if let Some(endpoint) = endpoint {
        builder = builder.endpoint(&endpoint);
    }
    if virtual_host_style {
        builder = builder.enable_virtual_host_style();
    }
    builder = match auth {
        ParsedS3Auth::Credentials(source) => configure_s3_source(builder, source),
        ParsedS3Auth::Unsigned => builder
            .skip_signature()
            .disable_config_load()
            .disable_ec2_metadata(),
        ParsedS3Auth::AssumeRole {
            role_arn,
            source,
            external_id,
            role_session_name,
            duration_seconds,
        } => {
            let mut builder = configure_s3_source(builder, source).role_arn(&role_arn);
            if let Some(external_id) = external_id {
                builder = builder.external_id(&external_id);
            }
            if let Some(role_session_name) = role_session_name {
                builder = builder.role_session_name(&role_session_name);
            }
            if let Some(duration_seconds) = duration_seconds {
                builder = builder.assume_role_duration_seconds(duration_seconds);
            }
            builder
        }
    };
    opendal::Operator::new(builder).map_err(construction_error)
}

unsafe extern "C" fn operator_new(
    scheme: *const BytesViewV1,
    config: *const KvV1,
    config_len: u64,
    out_operator: *mut *mut OperatorV1,
    out_info: *mut *mut OperatorInfoV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: required/optional outputs are validated and cleared before inputs.
        let outputs = [
            unsafe { clear_required_output(out_operator, ptr::null_mut()) },
            unsafe { clear_required_output(out_info, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<(Box<OperatorV1>, Box<OperatorInfoV1>)> {
            // SAFETY: scheme is required and copied as strict UTF-8.
            let scheme = unsafe { read_text(scheme, "scheme")? };
            // SAFETY: config array and all nested views are validated and copied.
            let config = unsafe { read_config(config, config_len)? };
            let runtime = runtime()?;
            let _guard = runtime.enter();
            let async_operator =
                opendal::Operator::via_iter(&scheme, config).map_err(construction_error)?;
            operator_handles(async_operator)
        })();
        match result {
            Ok((operator, info)) => {
                let operator = Box::into_raw(operator);
                let info = Box::into_raw(info);
                // SAFETY: both outputs were validated and remain exclusively writable.
                unsafe {
                    out_operator.write(operator);
                    out_info.write(info);
                }
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

#[cfg(feature = "profile-standard")]
unsafe extern "C" fn operator_s3(
    options: *const S3OptionsV1,
    out_operator: *mut *mut OperatorV1,
    out_info: *mut *mut OperatorInfoV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: required/optional outputs are validated and cleared before inputs.
        let outputs = [
            unsafe { clear_required_output(out_operator, ptr::null_mut()) },
            unsafe { clear_required_output(out_info, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<(Box<OperatorV1>, Box<OperatorInfoV1>)> {
            // SAFETY: the required options carrier and every selected nested
            // view are validated and copied during parsing.
            let options = unsafe { parse_s3_options(options)? };
            let runtime = runtime()?;
            let _guard = runtime.enter();
            let async_operator = build_s3_operator(options)?;
            operator_handles(async_operator)
        })();
        match result {
            Ok((operator, info)) => {
                // SAFETY: both outputs were validated and remain exclusively writable.
                unsafe {
                    out_operator.write(Box::into_raw(operator));
                    out_info.write(Box::into_raw(info));
                }
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_free(operator: *mut OperatorV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !operator.is_null() {
            // SAFETY: every non-null handle is uniquely owned and freed once.
            unsafe { drop(Box::from_raw(operator)) };
        }
    }));
}

#[cfg(feature = "layers-timeout-retry")]
fn positive_millis(value: u64, label: &str) -> CallResult<Duration> {
    if value == 0 {
        return Err(invalid_argument(format!(
            "{label} must be greater than zero"
        )));
    }
    Ok(Duration::from_millis(value))
}

#[cfg(feature = "layers-timeout-retry")]
fn retry_count(value: u32) -> CallResult<usize> {
    if value == 0 {
        return Err(invalid_argument("max_retries must be greater than zero"));
    }
    usize::try_from(value)
        .map_err(|_| invalid_argument("max_retries is not representable on this platform"))
}

#[cfg(feature = "layers-timeout-retry")]
fn rebuild_layered_operator(
    async_inner: opendal::Operator,
    layer_bits: u32,
) -> CallResult<(Box<OperatorV1>, Box<OperatorInfoV1>)> {
    let runtime = runtime()?;
    let _guard = runtime.enter();
    operator_handles_with_layers(async_inner, layer_bits)
}

#[cfg(feature = "layers-timeout-retry")]
unsafe extern "C" fn operator_with_timeout(
    operator: *const OperatorV1,
    operation_timeout_millis: u64,
    io_timeout_millis: u64,
    out_operator: *mut *mut OperatorV1,
    out_info: *mut *mut OperatorInfoV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        let outputs = [
            // SAFETY: output slots are validated and cleared before inputs are inspected.
            unsafe { clear_required_output(out_operator, ptr::null_mut()) },
            // SAFETY: output slots are validated and cleared before inputs are inspected.
            unsafe { clear_required_output(out_info, ptr::null_mut()) },
            // SAFETY: the optional error slot is cleared before work begins.
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<(Box<OperatorV1>, Box<OperatorInfoV1>)> {
            // SAFETY: opaque handle validity is a caller lifetime obligation.
            let operator = unsafe { borrow_required(operator)? };
            let operation_timeout =
                positive_millis(operation_timeout_millis, "operation_timeout_millis")?;
            let io_timeout = positive_millis(io_timeout_millis, "io_timeout_millis")?;
            if operator.layer_bits & OPERATOR_LAYER_TIMEOUT != 0 {
                return Err(invalid_argument("timeout layer is already installed"));
            }
            if operator.layer_bits & OPERATOR_LAYER_RETRY != 0 {
                return Err(invalid_argument(
                    "timeout layer must be installed before retry layer",
                ));
            }
            let layer = TimeoutLayer::new()
                .with_timeout(operation_timeout)
                .with_io_timeout(io_timeout);
            rebuild_layered_operator(
                operator.async_inner.clone().layer(layer),
                operator.layer_bits | OPERATOR_LAYER_TIMEOUT,
            )
        })();
        match result {
            Ok((operator, info)) => {
                // SAFETY: both required output slots were validated and remain writable.
                unsafe {
                    out_operator.write(Box::into_raw(operator));
                    out_info.write(Box::into_raw(info));
                }
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

#[cfg(feature = "layers-timeout-retry")]
unsafe extern "C" fn operator_with_retry(
    operator: *const OperatorV1,
    max_retries: u32,
    min_delay_millis: u64,
    max_delay_millis: u64,
    jitter: u32,
    out_operator: *mut *mut OperatorV1,
    out_info: *mut *mut OperatorInfoV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        let outputs = [
            // SAFETY: output slots are validated and cleared before inputs are inspected.
            unsafe { clear_required_output(out_operator, ptr::null_mut()) },
            // SAFETY: output slots are validated and cleared before inputs are inspected.
            unsafe { clear_required_output(out_info, ptr::null_mut()) },
            // SAFETY: the optional error slot is cleared before work begins.
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<(Box<OperatorV1>, Box<OperatorInfoV1>)> {
            // SAFETY: opaque handle validity is a caller lifetime obligation.
            let operator = unsafe { borrow_required(operator)? };
            if jitter > 1 {
                return abi_mismatch();
            }
            let max_retries = retry_count(max_retries)?;
            let min_delay = positive_millis(min_delay_millis, "min_delay_millis")?;
            let max_delay = positive_millis(max_delay_millis, "max_delay_millis")?;
            if min_delay > max_delay {
                return Err(invalid_argument(
                    "min_delay_millis must not exceed max_delay_millis",
                ));
            }
            if operator.layer_bits & OPERATOR_LAYER_RETRY != 0 {
                return Err(invalid_argument("retry layer is already installed"));
            }
            let mut layer = RetryLayer::new()
                .with_max_times(max_retries)
                .with_min_delay(min_delay)
                .with_max_delay(max_delay);
            if jitter != 0 {
                layer = layer.with_jitter();
            }
            rebuild_layered_operator(
                operator.async_inner.clone().layer(layer),
                operator.layer_bits | OPERATOR_LAYER_RETRY,
            )
        })();
        match result {
            Ok((operator, info)) => {
                // SAFETY: both required output slots were validated and remain writable.
                unsafe {
                    out_operator.write(Box::into_raw(operator));
                    out_info.write(Box::into_raw(info));
                }
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_check(
    operator: *mut OperatorV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: optional error output is cleared before validation/work.
        if let Err(failure) = unsafe { clear_error_output(out_error) } {
            return unsafe { finish_failure(failure, out_error) };
        }
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let operator = match unsafe { borrow_required(operator.cast_const()) } {
            Ok(operator) => operator,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        match operator.inner.check().map_err(opendal_error) {
            Ok(()) => STATUS_OK,
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_exists(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    out_exists: *mut u32,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: outputs are validated and cleared before input/work.
        let outputs = [unsafe { clear_required_output(out_exists, 0) }, unsafe {
            clear_error_output(out_error)
        }];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<u32> {
            // SAFETY: opaque handle and textual input are validated.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            operator
                .inner
                .exists(&path)
                .map(u32::from)
                .map_err(opendal_error)
        })();
        match result {
            Ok(exists) => {
                // SAFETY: output was validated above.
                unsafe { out_exists.write(exists) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_stat(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    options: *const StatOptionsV1,
    out_metadata: *mut *mut MetadataV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: outputs are validated and cleared before input/work.
        let outputs = [
            unsafe { clear_required_output(out_metadata, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<MetadataV1>> {
            // SAFETY: handle/text/options are validated and copied as needed.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let options = unsafe { parse_stat_options(options)? };
            let metadata = operator
                .inner
                .stat_options(&path, options)
                .map_err(opendal_error)?;
            Ok(Box::new(checked_metadata(metadata)?))
        })();
        match result {
            Ok(metadata) => {
                // SAFETY: output was validated above.
                unsafe { out_metadata.write(Box::into_raw(metadata)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_read(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    options: *const ReadOptionsV1,
    max_output_len: u64,
    out_buffer: *mut *mut BufferV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: outputs are validated and cleared before input/work.
        let outputs = [
            unsafe { clear_required_output(out_buffer, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<BufferV1>> {
            // SAFETY: handle/text/options are validated and copied as needed.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let options = unsafe { parse_read_options(options)? };
            let plan = whole_read_plan(options, max_output_len)?;
            let range = resolve_whole_read_range(&operator.inner, &path, &plan)?;
            let reader = operator
                .inner
                .reader_options(&path, plan.reader_options)
                .map_err(opendal_error)?;
            let iterator = reader.into_iterator(range).map_err(opendal_error)?;
            let mut output = Vec::new();
            for buffer in iterator {
                let buffer = buffer.map_err(opendal_error)?;
                #[cfg(test)]
                observe_whole_read_chunk(operator, u64::try_from(buffer.len()).unwrap_or(u64::MAX));
                append_whole_read_chunk(&mut output, buffer, plan.output_limit)?;
            }
            Ok(Box::new(BufferV1 { bytes: output }))
        })();
        match result {
            Ok(buffer) => {
                // SAFETY: output was validated above.
                unsafe { out_buffer.write(Box::into_raw(buffer)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_write(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    data: *const BytesViewV1,
    options: *const WriteOptionsV1,
    out_metadata: *mut *mut MetadataV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: outputs are validated and cleared before input/work.
        let outputs = [
            unsafe { clear_required_output(out_metadata, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<MetadataV1>> {
            // SAFETY: handle/text/binary/options inputs are validated and copied.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let data = unsafe { read_binary(data)? };
            let options = unsafe { parse_write_options(options)? };
            let metadata = operator
                .inner
                .write_options(&path, data, options)
                .map_err(opendal_error)?;
            Ok(Box::new(checked_metadata(metadata)?))
        })();
        match result {
            Ok(metadata) => {
                // SAFETY: output was validated above.
                unsafe { out_metadata.write(Box::into_raw(metadata)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_create_dir(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: optional error output is cleared before input/work.
        if let Err(failure) = unsafe { clear_error_output(out_error) } {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<()> {
            // SAFETY: handle and textual input are validated.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            operator.inner.create_dir(&path).map_err(opendal_error)
        })();
        match result {
            Ok(()) => STATUS_OK,
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_delete(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    options: *const DeleteOptionsV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: optional error output is cleared before input/work.
        if let Err(failure) = unsafe { clear_error_output(out_error) } {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<()> {
            // SAFETY: handle/text/options are validated and copied as needed.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let options = unsafe { parse_delete_options(options)? };
            operator
                .inner
                .delete_options(&path, options)
                .map_err(opendal_error)
        })();
        match result {
            Ok(()) => STATUS_OK,
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_copy(
    operator: *mut OperatorV1,
    source: *const BytesViewV1,
    destination: *const BytesViewV1,
    out_metadata: *mut *mut MetadataV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: outputs are validated and cleared before input/work.
        let outputs = [
            unsafe { clear_required_output(out_metadata, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<MetadataV1>> {
            // SAFETY: handle and both textual inputs are validated.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let source = unsafe { read_text(source, "source path")? };
            let destination = unsafe { read_text(destination, "destination path")? };
            let metadata = operator
                .inner
                .copy(&source, &destination)
                .map_err(opendal_error)?;
            Ok(Box::new(checked_metadata(metadata)?))
        })();
        match result {
            Ok(metadata) => {
                // SAFETY: output was validated above.
                unsafe { out_metadata.write(Box::into_raw(metadata)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_rename(
    operator: *mut OperatorV1,
    source: *const BytesViewV1,
    destination: *const BytesViewV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: optional error output is cleared before input/work.
        if let Err(failure) = unsafe { clear_error_output(out_error) } {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<()> {
            // SAFETY: handle and both textual inputs are validated.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let source = unsafe { read_text(source, "source path")? };
            let destination = unsafe { read_text(destination, "destination path")? };
            operator
                .inner
                .rename(&source, &destination)
                .map_err(opendal_error)
        })();
        match result {
            Ok(()) => STATUS_OK,
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_lister(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    options: *const ListOptionsV1,
    out_lister: *mut *mut ListerV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: outputs are validated and cleared before input/work.
        let outputs = [
            unsafe { clear_required_output(out_lister, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<ListerV1>> {
            // SAFETY: handle/text/options are validated and copied as needed.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let options = unsafe { parse_list_options(options)? };
            let lister = operator
                .inner
                .lister_options(&path, options)
                .map_err(opendal_error)?;
            Ok(Box::new(ListerV1 {
                state: Mutex::new(ListerStateV1::Open(lister)),
            }))
        })();
        match result {
            Ok(lister) => {
                // SAFETY: output was validated above.
                unsafe { out_lister.write(Box::into_raw(lister)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn lister_next(
    lister: *mut ListerV1,
    out_entry: *mut *mut EntryV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: outputs are validated and cleared before the handle is inspected.
        let outputs = [
            unsafe { clear_required_output(out_entry, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let lister = match unsafe { borrow_required(lister.cast_const()) } {
            Ok(lister) => lister,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        #[cfg(test)]
        let test_mode = take_lister_next_test_mode(lister);
        let mut state = match lister.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                let previous = std::mem::replace(&mut *state, ListerStateV1::Exhausted);
                if let ListerStateV1::Open(inner) = previous {
                    drop_blocking_lister_contained(inner);
                }
                // The uncertain state has been replaced with a deterministic
                // terminal state. Clear poison so this failure is reported
                // exactly once and later next calls return END.
                lister.state.clear_poison();
                drop(state);
                return unsafe {
                    finish_failure(
                        binding_error(
                            ERROR_UNEXPECTED,
                            "Unexpected",
                            "lister state lock was poisoned",
                        ),
                        out_error,
                    )
                };
            }
        };
        let current = std::mem::replace(&mut *state, ListerStateV1::Exhausted);
        let mut inner = match current {
            ListerStateV1::Open(inner) => inner,
            ListerStateV1::Exhausted => return STATUS_END,
            ListerStateV1::Closed => {
                *state = ListerStateV1::Closed;
                return unsafe {
                    finish_failure(
                        binding_error(ERROR_RESOURCE_CLOSED, "ResourceClosed", "lister is closed"),
                        out_error,
                    )
                };
            }
        };

        // Keep ownership of the native lister outside the unwind boundary. If
        // next, error conversion, or snapshot allocation panics, the state is
        // already Exhausted and the lister can be dropped under its own guard.
        let step = catch_unwind(AssertUnwindSafe(|| -> CallResult<Option<Box<EntryV1>>> {
            #[cfg(test)]
            match test_mode {
                TEST_LISTER_NEXT_ERROR => {
                    return Err(binding_error(
                        ERROR_UNEXPECTED,
                        "Unexpected",
                        "injected lister error",
                    ));
                }
                TEST_LISTER_NEXT_PANIC => panic!("injected lister panic"),
                _ => {}
            }
            match inner.next() {
                Some(Ok(entry)) => Ok(Some(Box::new(checked_entry(entry)?))),
                Some(Err(error)) => Err(opendal_error(error)),
                None => Ok(None),
            }
        }));
        match step {
            Ok(Ok(Some(entry))) => {
                *state = ListerStateV1::Open(inner);
                // SAFETY: the output was validated and remains exclusively writable.
                unsafe { out_entry.write(Box::into_raw(entry)) };
                STATUS_OK
            }
            Ok(Ok(None)) => {
                drop_blocking_lister_contained(inner);
                STATUS_END
            }
            Ok(Err(failure)) => {
                drop_blocking_lister_contained(inner);
                unsafe { finish_failure(failure, out_error) }
            }
            Err(_) => {
                drop_blocking_lister_contained(inner);
                STATUS_PANIC
            }
        }
    })
}

unsafe extern "C" fn lister_close(lister: *mut ListerV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if lister.is_null() {
            return;
        }
        // SAFETY: non-null opaque handle validity is the caller's obligation.
        let lister = unsafe { &*lister };
        close_lister_state(lister);
    }));
}

unsafe extern "C" fn lister_free(lister: *mut ListerV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if lister.is_null() {
            return;
        }
        // SAFETY: every non-null handle is uniquely owned and freed once.
        let lister = unsafe { Box::from_raw(lister) };
        close_lister_state(&lister);
        drop(lister);
    }));
}

unsafe extern "C" fn operator_reader(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    options: *const ReaderOptionsV1,
    out_reader: *mut *mut ReaderV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: outputs are validated and cleared before input/work.
        let outputs = [
            unsafe { clear_required_output(out_reader, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<ReaderV1>> {
            // SAFETY: handle/text/options are validated and copied as needed.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let options = unsafe { parse_reader_options(options)? };
            let reader = operator
                .inner
                .reader_options(&path, options)
                .map_err(opendal_error)?;
            Ok(Box::new(ReaderV1 {
                state: std::sync::RwLock::new(ReaderStateV1::Open(reader)),
            }))
        })();
        match result {
            Ok(reader) => {
                // SAFETY: output was validated above.
                unsafe { out_reader.write(Box::into_raw(reader)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn reader_read(
    reader: *mut ReaderV1,
    range: *const ByteRangeV1,
    max_output_len: u64,
    out_buffer: *mut *mut BufferV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: outputs are validated and cleared before the handle is inspected.
        let outputs = [
            unsafe { clear_required_output(out_buffer, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let reader = match unsafe { borrow_required(reader.cast_const()) } {
            Ok(reader) => reader,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        let result = (|| -> CallResult<Box<BufferV1>> {
            // SAFETY: the range is required, copied, and validated before use.
            let range = unsafe { parse_range(read_required(range)?)? };
            let negotiated_limit = max_output_len.min(MAX_OUTPUT_BYTES);
            match range {
                BytesRange::Range {
                    size: Some(size), ..
                }
                | BytesRange::Suffix { size }
                    if size > negotiated_limit =>
                {
                    return Err(buffer_too_large(
                        "reader range exceeds the negotiated output limit",
                    ));
                }
                _ => {}
            }
            #[cfg(test)]
            let test_mode = take_reader_read_test_mode(reader);
            let state = match reader.state.read() {
                Ok(state) => state,
                Err(poisoned) => {
                    drop(poisoned);
                    close_reader_state(reader);
                    return Err(binding_error(
                        ERROR_UNEXPECTED,
                        "Unexpected",
                        "reader state lock was poisoned",
                    ));
                }
            };
            let inner = match &*state {
                ReaderStateV1::Open(inner) => inner,
                ReaderStateV1::Closed => {
                    return Err(binding_error(
                        ERROR_RESOURCE_CLOSED,
                        "ResourceClosed",
                        "reader is closed",
                    ));
                }
            };
            #[cfg(test)]
            match test_mode {
                TEST_READER_READ_ERROR => {
                    return Err(binding_error(
                        ERROR_UNEXPECTED,
                        "Unexpected",
                        "injected reader error",
                    ));
                }
                TEST_READER_READ_PANIC => panic!("injected reader panic"),
                _ => {}
            }
            let buffer = inner.read(range).map_err(opendal_error)?;
            checked_read_buffer(buffer, negotiated_limit, "reader result")
        })();
        match result {
            Ok(buffer) => {
                // SAFETY: output was validated above.
                unsafe { out_buffer.write(Box::into_raw(buffer)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    }));
    match outcome {
        Ok(status) => status,
        Err(_) => {
            if !reader.is_null() && is_aligned(reader) {
                // SAFETY: non-null live handle validity remains the caller's obligation.
                close_reader_state(unsafe { &*reader });
            }
            STATUS_PANIC
        }
    }
}

unsafe extern "C" fn reader_close(reader: *mut ReaderV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if reader.is_null() {
            return;
        }
        // SAFETY: non-null opaque handle validity is the caller's obligation.
        close_reader_state(unsafe { &*reader });
    }));
}

unsafe extern "C" fn reader_free(reader: *mut ReaderV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if reader.is_null() {
            return;
        }
        // SAFETY: every non-null handle is uniquely owned and freed once.
        let reader = unsafe { Box::from_raw(reader) };
        close_reader_state(&reader);
        drop(reader);
    }));
}

unsafe extern "C" fn operator_read_stream(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    options: *const ReadStreamOptionsV1,
    out_stream: *mut *mut ReadStreamV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: outputs are validated and cleared before input or native work.
        let outputs = [
            unsafe { clear_required_output(out_stream, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<ReadStreamV1>> {
            // SAFETY: the handle, text, and options are validated and copied.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let parsed = unsafe { parse_read_stream_options(options)? };
            let reader = operator
                .inner
                .reader_options(&path, parsed.reader)
                .map_err(opendal_error)?;
            let iterator = reader.into_iterator(parsed.range).map_err(opendal_error)?;
            Ok(Box::new(ReadStreamV1 {
                state: Mutex::new(ReadStreamStateV1::Open(Box::new(iterator))),
                chunk_size: parsed.chunk_size,
            }))
        })();
        match result {
            Ok(stream) => {
                // SAFETY: the output slot was validated above.
                unsafe { out_stream.write(Box::into_raw(stream)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn read_stream_next(
    stream: *mut ReadStreamV1,
    max_output_len: u64,
    out_buffer: *mut *mut BufferV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: outputs are validated and cleared before the handle is inspected.
        let outputs = [
            unsafe { clear_required_output(out_buffer, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let stream = match unsafe { borrow_required(stream.cast_const()) } {
            Ok(stream) => stream,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        let mut state = match stream.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                drop(poisoned);
                let failure = read_stream_lock_failure(stream);
                return unsafe { finish_failure(failure, out_error) };
            }
        };
        let current = std::mem::replace(&mut *state, ReadStreamStateV1::Failed);
        let mut iterator = match current {
            ReadStreamStateV1::Open(iterator) => iterator,
            ReadStreamStateV1::End => {
                *state = ReadStreamStateV1::End;
                return STATUS_END;
            }
            terminal @ (ReadStreamStateV1::Failed | ReadStreamStateV1::Closed) => {
                *state = terminal;
                return unsafe {
                    finish_failure(
                        binding_error(
                            ERROR_RESOURCE_CLOSED,
                            "ResourceClosed",
                            "read stream is closed",
                        ),
                        out_error,
                    )
                };
            }
        };
        let step = catch_unwind(AssertUnwindSafe(|| iterator.next()));
        match step {
            Ok(Some(Ok(buffer))) => {
                let limit = max_output_len.min(stream.chunk_size);
                match checked_read_buffer(buffer, limit, "read stream chunk") {
                    Ok(buffer) => {
                        *state = ReadStreamStateV1::Open(iterator);
                        // SAFETY: the output slot was validated above.
                        unsafe { out_buffer.write(Box::into_raw(buffer)) };
                        STATUS_OK
                    }
                    Err(failure) => unsafe { finish_failure(failure, out_error) },
                }
            }
            Ok(Some(Err(error))) => unsafe { finish_failure(opendal_error(error), out_error) },
            Ok(None) => {
                *state = ReadStreamStateV1::End;
                STATUS_END
            }
            Err(_) => STATUS_PANIC,
        }
    }));
    match outcome {
        Ok(status) => status,
        Err(_) => {
            if !stream.is_null() && is_aligned(stream) {
                // SAFETY: non-null live handle validity remains the caller's obligation.
                fail_read_stream_state(unsafe { &*stream });
            }
            STATUS_PANIC
        }
    }
}

unsafe extern "C" fn read_stream_close(stream: *mut ReadStreamV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if stream.is_null() {
            return;
        }
        // SAFETY: non-null opaque handle validity is the caller's obligation.
        close_read_stream_state(unsafe { &*stream });
    }));
}

unsafe extern "C" fn read_stream_free(stream: *mut ReadStreamV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if stream.is_null() {
            return;
        }
        // SAFETY: every non-null handle is uniquely owned and freed once.
        let stream = unsafe { Box::from_raw(stream) };
        close_read_stream_state(&stream);
        drop(stream);
    }));
}

unsafe extern "C" fn operator_writer(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    options: *const WriteOptionsV1,
    out_writer: *mut *mut WriterV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        // SAFETY: outputs are validated and cleared before input/work.
        let outputs = [
            unsafe { clear_required_output(out_writer, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<WriterV1>> {
            // SAFETY: handle/text/options are validated and copied as needed.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let options = unsafe { parse_write_options(options)? };
            let writer = runtime()?
                .block_on(operator.async_inner.writer_options(&path, options))
                .map_err(opendal_error)?;
            Ok(Box::new(WriterV1 {
                state: Mutex::new(WriterStateV1::Open(writer)),
                changed: Condvar::new(),
            }))
        })();
        match result {
            Ok(writer) => {
                // SAFETY: output was validated above.
                unsafe { out_writer.write(Box::into_raw(writer)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn writer_write(
    writer: *mut WriterV1,
    data: *const BytesViewV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: optional error output is cleared before input/work.
        if let Err(failure) = unsafe { clear_error_output(out_error) } {
            return unsafe { finish_failure(failure, out_error) };
        }
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let writer = match unsafe { borrow_required(writer.cast_const()) } {
            Ok(writer) => writer,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        // SAFETY: the binary carrier is copied before entering native state.
        let data = match unsafe { read_binary(data) } {
            Ok(data) => data,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        #[cfg(test)]
        let test_mode = take_writer_call_test_mode(writer);
        let runtime = match runtime() {
            Ok(runtime) => runtime,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        let mut inner = match take_writer_for_call(writer, false) {
            Ok(Some(inner)) => inner,
            Ok(None) => unreachable!("non-abort writer call cannot observe idempotent abort"),
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        // State is Busy while async work runs, so no mutex guard crosses block_on.
        let step = catch_unwind(AssertUnwindSafe(|| -> CallResult<()> {
            #[cfg(test)]
            match test_mode {
                TEST_WRITER_WRITE_ERROR => {
                    return Err(binding_error(
                        ERROR_UNEXPECTED,
                        "Unexpected",
                        "injected writer write error",
                    ));
                }
                TEST_WRITER_WRITE_PANIC => panic!("injected writer write panic"),
                _ => {}
            }
            runtime.block_on(inner.write(data)).map_err(opendal_error)
        }));
        match step {
            Ok(Ok(())) => match set_writer_state(writer, WriterStateV1::Open(inner)) {
                Ok(()) => STATUS_OK,
                Err(failure) => unsafe { finish_failure(failure, out_error) },
            },
            Ok(Err(failure)) => {
                let _ = set_writer_state(writer, WriterStateV1::Failed);
                drop_async_writer_contained(inner);
                unsafe { finish_failure(failure, out_error) }
            }
            Err(_) => {
                let _ = set_writer_state(writer, WriterStateV1::Failed);
                drop_async_writer_contained(inner);
                STATUS_PANIC
            }
        }
    }));
    match outcome {
        Ok(status) => status,
        Err(_) => {
            if !writer.is_null() && is_aligned(writer) {
                // SAFETY: non-null live handle validity remains the caller's obligation.
                fail_writer_state(unsafe { &*writer });
            }
            STATUS_PANIC
        }
    }
}

unsafe extern "C" fn writer_close(
    writer: *mut WriterV1,
    out_metadata: *mut *mut MetadataV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: outputs are validated and cleared before the handle is inspected.
        let outputs = [
            unsafe { clear_required_output(out_metadata, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let writer = match unsafe { borrow_required(writer.cast_const()) } {
            Ok(writer) => writer,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        #[cfg(test)]
        let test_mode = take_writer_call_test_mode(writer);
        let runtime = match runtime() {
            Ok(runtime) => runtime,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        let mut inner = match take_writer_for_call(writer, false) {
            Ok(Some(inner)) => inner,
            Ok(None) => unreachable!("non-abort writer call cannot observe idempotent abort"),
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        // The first close attempt is terminal before native close begins.
        let step = catch_unwind(AssertUnwindSafe(|| -> CallResult<Box<MetadataV1>> {
            #[cfg(test)]
            match test_mode {
                TEST_WRITER_CLOSE_ERROR => {
                    return Err(binding_error(
                        ERROR_UNEXPECTED,
                        "Unexpected",
                        "injected writer close error",
                    ));
                }
                TEST_WRITER_CLOSE_PANIC => panic!("injected writer close panic"),
                _ => {}
            }
            #[cfg(test)]
            TEST_WRITER_NATIVE_CLOSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let metadata = runtime.block_on(inner.close()).map_err(opendal_error)?;
            Ok(Box::new(checked_metadata(metadata)?))
        }));
        match step {
            Ok(Ok(metadata)) => {
                if let Err(failure) = set_writer_state(writer, WriterStateV1::Finished) {
                    drop_async_writer_contained(inner);
                    return unsafe { finish_failure(failure, out_error) };
                }
                drop_async_writer_contained(inner);
                // SAFETY: output was validated above.
                unsafe { out_metadata.write(Box::into_raw(metadata)) };
                STATUS_OK
            }
            Ok(Err(failure)) => {
                let _ = set_writer_state(writer, WriterStateV1::Failed);
                drop_async_writer_contained(inner);
                unsafe { finish_failure(failure, out_error) }
            }
            Err(_) => {
                let _ = set_writer_state(writer, WriterStateV1::Failed);
                drop_async_writer_contained(inner);
                STATUS_PANIC
            }
        }
    }));
    match outcome {
        Ok(status) => status,
        Err(_) => {
            if !writer.is_null() && is_aligned(writer) {
                // SAFETY: non-null live handle validity remains the caller's obligation.
                fail_writer_state(unsafe { &*writer });
            }
            STATUS_PANIC
        }
    }
}

unsafe extern "C" fn writer_abort(writer: *mut WriterV1, out_error: *mut *mut ErrorV1) -> Status {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: optional error output is cleared before the handle is inspected.
        if let Err(failure) = unsafe { clear_error_output(out_error) } {
            return unsafe { finish_failure(failure, out_error) };
        }
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let writer = match unsafe { borrow_required(writer.cast_const()) } {
            Ok(writer) => writer,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        #[cfg(test)]
        let test_mode = take_writer_call_test_mode(writer);
        let runtime = match runtime() {
            Ok(runtime) => runtime,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        let mut inner = match take_writer_for_call(writer, true) {
            Ok(Some(inner)) => inner,
            Ok(None) => return STATUS_OK,
            Err(failure) => return unsafe { finish_failure(failure, out_error) },
        };
        // State is Busy while async work runs, so finish/write/abort races
        // linearize before one terminal operation enters OpenDAL.
        let step = catch_unwind(AssertUnwindSafe(|| -> CallResult<()> {
            #[cfg(test)]
            match test_mode {
                TEST_WRITER_ABORT_ERROR => {
                    return Err(binding_error(
                        ERROR_UNEXPECTED,
                        "Unexpected",
                        "injected writer abort error",
                    ));
                }
                TEST_WRITER_ABORT_PANIC => panic!("injected writer abort panic"),
                _ => {}
            }
            #[cfg(test)]
            TEST_WRITER_NATIVE_ABORTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            runtime.block_on(inner.abort()).map_err(opendal_error)
        }));
        match step {
            Ok(Ok(())) => {
                let result = set_writer_state(writer, WriterStateV1::Aborted);
                drop_async_writer_contained(inner);
                match result {
                    Ok(()) => STATUS_OK,
                    Err(failure) => unsafe { finish_failure(failure, out_error) },
                }
            }
            Ok(Err(failure)) => {
                let _ = set_writer_state(writer, WriterStateV1::Failed);
                drop_async_writer_contained(inner);
                unsafe { finish_failure(failure, out_error) }
            }
            Err(_) => {
                let _ = set_writer_state(writer, WriterStateV1::Failed);
                drop_async_writer_contained(inner);
                STATUS_PANIC
            }
        }
    }));
    match outcome {
        Ok(status) => status,
        Err(_) => {
            if !writer.is_null() && is_aligned(writer) {
                // SAFETY: non-null live handle validity remains the caller's obligation.
                fail_writer_state(unsafe { &*writer });
            }
            STATUS_PANIC
        }
    }
}

unsafe extern "C" fn operator_presign_read(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    options: *const ReadOptionsV1,
    expires_in_seconds: u64,
    out_request: *mut *mut PresignedRequestV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        let outputs = [
            unsafe { clear_required_output(out_request, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<PresignedRequestV1>> {
            // SAFETY: handle, path, and option carriers are validated and copied.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let options = unsafe { parse_read_options(options)? };
            let expiry = presign_expiry(expires_in_seconds)?;
            let request = operator
                .inner
                .presign_read_options(&path, expiry, options)
                .map_err(opendal_error)?;
            Ok(Box::new(checked_presigned_request(request)?))
        })();
        match result {
            Ok(request) => {
                // SAFETY: output was validated and remains exclusively writable.
                unsafe { out_request.write(Box::into_raw(request)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_presign_write(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    options: *const WriteOptionsV1,
    expires_in_seconds: u64,
    out_request: *mut *mut PresignedRequestV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        let outputs = [
            unsafe { clear_required_output(out_request, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<PresignedRequestV1>> {
            // SAFETY: handle, path, and option carriers are validated and copied.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let options = unsafe { parse_write_options(options)? };
            let expiry = presign_expiry(expires_in_seconds)?;
            let request = operator
                .inner
                .presign_write_options(&path, expiry, options)
                .map_err(opendal_error)?;
            Ok(Box::new(checked_presigned_request(request)?))
        })();
        match result {
            Ok(request) => {
                // SAFETY: output was validated and remains exclusively writable.
                unsafe { out_request.write(Box::into_raw(request)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn operator_presign_stat(
    operator: *mut OperatorV1,
    path: *const BytesViewV1,
    options: *const StatOptionsV1,
    expires_in_seconds: u64,
    out_request: *mut *mut PresignedRequestV1,
    out_error: *mut *mut ErrorV1,
) -> Status {
    catch_status(|| {
        let outputs = [
            unsafe { clear_required_output(out_request, ptr::null_mut()) },
            unsafe { clear_error_output(out_error) },
        ];
        if let Err(failure) = combine_output_validation(outputs) {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<PresignedRequestV1>> {
            // SAFETY: handle, path, and option carriers are validated and copied.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let options = unsafe { parse_stat_options(options)? };
            let expiry = presign_expiry(expires_in_seconds)?;
            let request = operator
                .inner
                .presign_stat_options(&path, expiry, options)
                .map_err(opendal_error)?;
            Ok(Box::new(checked_presigned_request(request)?))
        })();
        match result {
            Ok(request) => {
                // SAFETY: output was validated and remains exclusively writable.
                unsafe { out_request.write(Box::into_raw(request)) };
                STATUS_OK
            }
            Err(failure) => unsafe { finish_failure(failure, out_error) },
        }
    })
}

unsafe extern "C" fn presigned_request_view(
    request: *const PresignedRequestV1,
    output: *mut PresignedRequestViewV1,
) -> Status {
    catch_status(|| {
        // SAFETY: output is validated and cleared before the handle is inspected.
        let header = match unsafe { prepare_view(output) } {
            Ok(header) => header,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let request = match unsafe { borrow_required(request) } {
            Ok(request) => request,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        let header_count = match u64::try_from(request.headers.len()) {
            Ok(count) => count,
            Err(_) => return STATUS_PANIC,
        };
        let view = PresignedRequestViewV1 {
            struct_size: header.struct_size,
            struct_version: header.struct_version,
            reserved0: 0,
            method: string_view(&request.method),
            uri: string_view(&request.uri),
            header_count,
        };
        // SAFETY: prepare_view validated complete writable storage.
        unsafe { output.write(view) };
        STATUS_OK
    })
}

unsafe extern "C" fn presigned_request_header_view(
    request: *const PresignedRequestV1,
    index: u64,
    output: *mut PresignedHeaderViewV1,
) -> Status {
    catch_status(|| {
        // SAFETY: output is validated and cleared before the handle is inspected.
        let view_header = match unsafe { prepare_view(output) } {
            Ok(header) => header,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        // SAFETY: opaque handle validity is a caller lifetime obligation.
        let request = match unsafe { borrow_required(request) } {
            Ok(request) => request,
            Err(failure) => return unsafe { finish_failure(failure, ptr::null_mut()) },
        };
        let Some(header) = usize::try_from(index)
            .ok()
            .and_then(|index| request.headers.get(index))
        else {
            return STATUS_END;
        };
        let view = PresignedHeaderViewV1 {
            struct_size: view_header.struct_size,
            struct_version: view_header.struct_version,
            reserved0: 0,
            name: string_view(&header.name),
            value: bytes_view(&header.value),
        };
        // SAFETY: prepare_view validated complete writable storage.
        unsafe { output.write(view) };
        STATUS_OK
    })
}

unsafe extern "C" fn presigned_request_free(request: *mut PresignedRequestV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !request.is_null() {
            // SAFETY: every non-null handle is uniquely owned and freed once.
            unsafe { drop(Box::from_raw(request)) };
        }
    }));
}

unsafe extern "C" fn writer_free(writer: *mut WriterV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if writer.is_null() {
            return;
        }
        // SAFETY: every non-null handle is uniquely owned and freed once.
        let writer = unsafe { Box::from_raw(writer) };
        // This only drops the native Writer; it never invokes close/finish.
        fail_writer_state(&writer);
        drop(writer);
    }));
}

#[cfg(feature = "layers-timeout-retry")]
const OPERATOR_WITH_TIMEOUT_ENTRY: Option<OperatorWithTimeoutFn> = Some(operator_with_timeout);
#[cfg(not(feature = "layers-timeout-retry"))]
const OPERATOR_WITH_TIMEOUT_ENTRY: Option<OperatorWithTimeoutFn> = None;

#[cfg(feature = "layers-timeout-retry")]
const OPERATOR_WITH_RETRY_ENTRY: Option<OperatorWithRetryFn> = Some(operator_with_retry);
#[cfg(not(feature = "layers-timeout-retry"))]
const OPERATOR_WITH_RETRY_ENTRY: Option<OperatorWithRetryFn> = None;

fn stage_api() -> Option<ApiV1> {
    Some(ApiV1 {
        struct_size: 0,
        requested_major: 0,
        library_struct_size: u32::try_from(size_of::<ApiV1>()).ok()?,
        library_minor: ABI_MINOR,
        library_patch: ABI_PATCH,
        reserved0: 0,
        feature_bits: FEATURE_BASE
            | FEATURE_WHOLE_OBJECT
            | FEATURE_LISTING
            | FEATURE_RANDOM_READER
            | FEATURE_CHUNKED_WRITER
            | FEATURE_READ_STREAM
            | FEATURE_WRITER_ABORT
            | FEATURE_PRESIGN
            | PROFILE_FEATURE_BITS
            | LAYER_FEATURE_BITS,
        max_output_bytes: MAX_OUTPUT_BYTES,
        library_info: Some(library_info),
        error_view: Some(error_view),
        error_free: Some(error_free),
        buffer_len: Some(buffer_len),
        buffer_copy: Some(buffer_copy),
        buffer_free: Some(buffer_free),
        metadata_view: Some(metadata_view),
        metadata_free: Some(metadata_free),
        entry_view: Some(entry_view),
        entry_metadata_view: Some(entry_metadata_view),
        entry_free: Some(entry_free),
        operator_info_view: Some(operator_info_view),
        operator_info_free: Some(operator_info_free),
        operator_new: Some(operator_new),
        operator_free: Some(operator_free),
        operator_check: Some(operator_check),
        operator_exists: Some(operator_exists),
        operator_stat: Some(operator_stat),
        operator_read: Some(operator_read),
        operator_write: Some(operator_write),
        operator_create_dir: Some(operator_create_dir),
        operator_delete: Some(operator_delete),
        operator_copy: Some(operator_copy),
        operator_rename: Some(operator_rename),
        operator_lister: Some(operator_lister),
        lister_next: Some(lister_next),
        lister_close: Some(lister_close),
        lister_free: Some(lister_free),
        operator_reader: Some(operator_reader),
        reader_read: Some(reader_read),
        reader_close: Some(reader_close),
        reader_free: Some(reader_free),
        operator_writer: Some(operator_writer),
        writer_write: Some(writer_write),
        writer_close: Some(writer_close),
        writer_free: Some(writer_free),
        operator_read_stream: Some(operator_read_stream),
        read_stream_next: Some(read_stream_next),
        read_stream_close: Some(read_stream_close),
        read_stream_free: Some(read_stream_free),
        writer_abort: Some(writer_abort),
        #[cfg(feature = "profile-standard")]
        operator_s3: Some(operator_s3),
        #[cfg(not(feature = "profile-standard"))]
        operator_s3: None,
        operator_presign_read: Some(operator_presign_read),
        operator_presign_write: Some(operator_presign_write),
        operator_presign_stat: Some(operator_presign_stat),
        presigned_request_view: Some(presigned_request_view),
        presigned_request_header_view: Some(presigned_request_header_view),
        presigned_request_free: Some(presigned_request_free),
        operator_with_timeout: OPERATOR_WITH_TIMEOUT_ENTRY,
        operator_with_retry: OPERATOR_WITH_RETRY_ENTRY,
    })
}

unsafe fn install_api(base: *mut u8, caller_size: usize, staged: &ApiV1) {
    let limit = caller_size.min(size_of::<ApiV1>());
    // SAFETY: validated caller storage covers caller_size bytes. Clearing starts
    // after the permanent input prefix and never reaches the caller tail.
    unsafe { ptr::write_bytes(base.add(API_INPUT_SIZE), 0, limit - API_INPUT_SIZE) };

    macro_rules! install_field {
        ($field:ident) => {{
            let offset = offset_of!(ApiV1, $field);
            let size = size_of_val(&staged.$field);
            if offset + size <= limit {
                // SAFETY: the whole field is covered by both tables and the
                // staged source has the exact repr(C) field representation.
                unsafe {
                    ptr::copy_nonoverlapping(
                        ptr::addr_of!(staged.$field).cast::<u8>(),
                        base.add(offset),
                        size,
                    );
                }
            }
        }};
    }

    install_field!(library_struct_size);
    install_field!(library_minor);
    install_field!(library_patch);
    install_field!(reserved0);
    install_field!(feature_bits);
    install_field!(max_output_bytes);
    install_field!(library_info);
    install_field!(error_view);
    install_field!(error_free);
    install_field!(buffer_len);
    install_field!(buffer_copy);
    install_field!(buffer_free);
    install_field!(metadata_view);
    install_field!(metadata_free);
    install_field!(entry_view);
    install_field!(entry_metadata_view);
    install_field!(entry_free);
    install_field!(operator_info_view);
    install_field!(operator_info_free);
    install_field!(operator_new);
    install_field!(operator_free);
    install_field!(operator_check);
    install_field!(operator_exists);
    install_field!(operator_stat);
    install_field!(operator_read);
    install_field!(operator_write);
    install_field!(operator_create_dir);
    install_field!(operator_delete);
    install_field!(operator_copy);
    install_field!(operator_rename);
    install_field!(operator_lister);
    install_field!(lister_next);
    install_field!(lister_close);
    install_field!(lister_free);
    install_field!(operator_reader);
    install_field!(reader_read);
    install_field!(reader_close);
    install_field!(reader_free);
    install_field!(operator_writer);
    install_field!(writer_write);
    install_field!(writer_close);
    install_field!(writer_free);
    install_field!(operator_read_stream);
    install_field!(read_stream_next);
    install_field!(read_stream_close);
    install_field!(read_stream_free);
    install_field!(writer_abort);
    install_field!(operator_s3);
    install_field!(operator_presign_read);
    install_field!(operator_presign_write);
    install_field!(operator_presign_stat);
    install_field!(presigned_request_view);
    install_field!(presigned_request_header_view);
    install_field!(presigned_request_free);
    install_field!(operator_with_timeout);
    install_field!(operator_with_retry);
}

/// Negotiate the stable v1 function table.
///
/// # Safety
///
/// `inout_api` must satisfy the pointer and storage contract documented by
/// `native/include/opendal_mbt.h`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opendal_mbt_get_api(inout_api: *mut c_void) -> Status {
    if inout_api.is_null() || !is_aligned(inout_api.cast::<ApiV1>()) {
        return STATUS_ABI_MISMATCH;
    }
    let base = inout_api.cast::<u8>();
    // SAFETY: the bootstrap contract always provides the readable input prefix,
    // even if the declared size itself is invalidly small.
    let caller_size_u32 = unsafe { base.cast::<u32>().read() };
    // SAFETY: requested_major is the second u32 in the permanent prefix.
    let requested_major = unsafe { base.add(size_of::<u32>()).cast::<u32>().read() };
    let caller_size = match usize::try_from(caller_size_u32) {
        Ok(size) if size <= isize::MAX as usize => size,
        _ => return STATUS_ABI_MISMATCH,
    };
    if caller_size < API_PREFIX_SIZE
        || requested_major != ABI_MAJOR
        || base.addr().checked_add(caller_size).is_none()
    {
        return STATUS_ABI_MISMATCH;
    }

    // Stage the complete table before any caller payload is modified.
    let staged = match catch_unwind(AssertUnwindSafe(stage_api)) {
        Ok(Some(staged)) => staged,
        Ok(None) | Err(_) => return STATUS_PANIC,
    };

    // SAFETY: all validation and potentially panicking staging work is done;
    // install_api performs only bounded raw clears/copies of complete fields.
    unsafe { install_api(base, caller_size, &staged) };
    STATUS_OK
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;
    use std::ptr::NonNull;

    fn bytes(value: &[u8]) -> BytesViewV1 {
        BytesViewV1 {
            data: if value.is_empty() {
                ptr::null()
            } else {
                value.as_ptr()
            },
            len: u64::try_from(value.len()).expect("test input length fits u64"),
        }
    }

    fn api() -> ApiV1 {
        let mut api = MaybeUninit::<ApiV1>::zeroed();
        // SAFETY: write the permanent input prefix before calling bootstrap.
        unsafe {
            let pointer = api.as_mut_ptr();
            ptr::addr_of_mut!((*pointer).struct_size)
                .write(u32::try_from(size_of::<ApiV1>()).expect("table fits u32"));
            ptr::addr_of_mut!((*pointer).requested_major).write(ABI_MAJOR);
            assert_eq!(opendal_mbt_get_api(pointer.cast()), STATUS_OK);
            api.assume_init()
        }
    }

    #[cfg(feature = "profile-standard")]
    fn s3_options() -> S3OptionsV1 {
        S3OptionsV1 {
            struct_size: size_of::<S3OptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: S3_ENDPOINT_PRESENT,
            flags: 0,
            auth_kind: S3_AUTH_UNSIGNED,
            source_kind: S3_SOURCE_DEFAULT_CHAIN,
            bucket: bytes(b"moonbit-binding-test"),
            region: bytes(b"us-east-1"),
            root: bytes(b""),
            endpoint: bytes(b"http://127.0.0.1:9000"),
            access_key_id: bytes(b""),
            secret_access_key: bytes(b""),
            session_token: bytes(b""),
            role_arn: bytes(b""),
            external_id: bytes(b""),
            role_session_name: bytes(b""),
            assume_role_duration_seconds: 0,
            reserved0: 0,
        }
    }

    #[cfg(feature = "profile-standard")]
    fn assert_s3_constructs(api: &ApiV1, options: &S3OptionsV1) {
        let mut operator = NonNull::<OperatorV1>::dangling().as_ptr();
        let mut info = NonNull::<OperatorInfoV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the complete carrier, all nested views, and output slots stay live.
        let status = unsafe {
            api.operator_s3.expect("S3 constructor is installed")(
                options,
                &mut operator,
                &mut info,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!operator.is_null());
        assert!(!info.is_null());
        assert!(error.is_null());

        let mut view = OperatorInfoViewV1 {
            struct_size: size_of::<OperatorInfoViewV1>() as u32,
            struct_version: STRUCT_VERSION,
            reserved0: 0,
            scheme: bytes(b""),
            root: bytes(b""),
            name: bytes(b""),
            capability: CapabilityV1::default(),
        };
        // SAFETY: both returned handles are owned and the view is complete storage.
        unsafe {
            assert_eq!(
                api.operator_info_view.expect("BASE info view is installed")(info, &mut view),
                STATUS_OK,
            );
            assert_eq!(copy_view(view.scheme), b"s3");
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    fn memory_operator(api: &ApiV1) -> (*mut OperatorV1, *mut OperatorInfoV1) {
        let mut operator = ptr::null_mut();
        let mut info = ptr::null_mut();
        let mut error = ptr::null_mut();
        let scheme = bytes(b"memory");
        // SAFETY: all pointers and views obey the C ABI contract.
        let status = unsafe {
            api.operator_new.expect("BASE constructor is installed")(
                &scheme,
                ptr::null(),
                0,
                &mut operator,
                &mut info,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!operator.is_null());
        assert!(!info.is_null());
        assert!(error.is_null());
        (operator, info)
    }

    #[cfg(feature = "layers-timeout-retry")]
    fn timeout_operator(
        api: &ApiV1,
        operator: *const OperatorV1,
        operation_timeout_millis: u64,
        io_timeout_millis: u64,
    ) -> Result<*mut OperatorV1, u32> {
        let mut output = NonNull::<OperatorV1>::dangling().as_ptr();
        let mut info = NonNull::<OperatorInfoV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the input handle and all output slots remain live for this call.
        let status = unsafe {
            api.operator_with_timeout
                .expect("LAYERS timeout constructor is installed")(
                operator,
                operation_timeout_millis,
                io_timeout_millis,
                &mut output,
                &mut info,
                &mut error,
            )
        };
        match status {
            STATUS_OK => {
                assert!(!output.is_null());
                assert!(!info.is_null());
                assert!(error.is_null());
                // SAFETY: this helper owns the returned immutable info snapshot.
                unsafe { api.operator_info_free.expect("BASE info free is installed")(info) };
                Ok(output)
            }
            STATUS_ERROR => {
                assert!(output.is_null());
                assert!(info.is_null());
                assert!(!error.is_null());
                Err(take_error_kind(api, error))
            }
            other => {
                assert!(output.is_null());
                assert!(info.is_null());
                assert!(error.is_null());
                Err(other)
            }
        }
    }

    #[cfg(feature = "layers-timeout-retry")]
    fn retry_operator(
        api: &ApiV1,
        operator: *const OperatorV1,
        max_retries: u32,
        min_delay_millis: u64,
        max_delay_millis: u64,
        jitter: u32,
    ) -> Result<*mut OperatorV1, u32> {
        let mut output = NonNull::<OperatorV1>::dangling().as_ptr();
        let mut info = NonNull::<OperatorInfoV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the input handle and all output slots remain live for this call.
        let status = unsafe {
            api.operator_with_retry
                .expect("LAYERS retry constructor is installed")(
                operator,
                max_retries,
                min_delay_millis,
                max_delay_millis,
                jitter,
                &mut output,
                &mut info,
                &mut error,
            )
        };
        match status {
            STATUS_OK => {
                assert!(!output.is_null());
                assert!(!info.is_null());
                assert!(error.is_null());
                // SAFETY: this helper owns the returned immutable info snapshot.
                unsafe { api.operator_info_free.expect("BASE info free is installed")(info) };
                Ok(output)
            }
            STATUS_ERROR => {
                assert!(output.is_null());
                assert!(info.is_null());
                assert!(!error.is_null());
                Err(take_error_kind(api, error))
            }
            other => {
                assert!(output.is_null());
                assert!(info.is_null());
                assert!(error.is_null());
                Err(other)
            }
        }
    }

    fn filesystem_operator(
        api: &ApiV1,
        root: &std::path::Path,
    ) -> (*mut OperatorV1, *mut OperatorInfoV1) {
        let mut operator = ptr::null_mut();
        let mut info = ptr::null_mut();
        let mut error = ptr::null_mut();
        let scheme = bytes(b"fs");
        let root = root.to_string_lossy();
        let config = [KvV1 {
            key: bytes(b"root"),
            value: bytes(root.as_bytes()),
        }];
        // SAFETY: all views, the config element, and output slots are live.
        let status = unsafe {
            api.operator_new.expect("BASE constructor is installed")(
                &scheme,
                config.as_ptr(),
                1,
                &mut operator,
                &mut info,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!operator.is_null());
        assert!(!info.is_null());
        assert!(error.is_null());
        (operator, info)
    }

    fn take_error_kind(api: &ApiV1, error: *mut ErrorV1) -> u32 {
        assert!(!error.is_null());
        let mut view = ErrorViewV1 {
            struct_size: size_of::<ErrorViewV1>() as u32,
            struct_version: STRUCT_VERSION,
            kind: 0,
            status: 0,
            kind_name: bytes(b""),
            message: bytes(b""),
        };
        // SAFETY: error is an owned ABI handle and view is complete writable storage.
        unsafe {
            assert_eq!(
                api.error_view.expect("BASE error view is installed")(error, &mut view),
                STATUS_OK
            );
            api.error_free.expect("BASE error free is installed")(error);
        }
        view.kind
    }

    fn write_memory_object(api: &ApiV1, operator: *mut OperatorV1, path: &[u8], data: &[u8]) {
        let path = bytes(path);
        let data = bytes(data);
        let mut metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: all carriers and output slots are live for the duration of the call.
        let status = unsafe {
            api.operator_write.expect("WHOLE_OBJECT write is installed")(
                operator,
                &path,
                &data,
                ptr::null(),
                &mut metadata,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!metadata.is_null());
        assert!(error.is_null());
        // SAFETY: the successful call returned one uniquely owned metadata handle.
        unsafe { api.metadata_free.expect("BASE metadata free is installed")(metadata) };
    }

    fn memory_object_exists(api: &ApiV1, operator: *mut OperatorV1, path: &[u8]) -> bool {
        let path = bytes(path);
        let mut exists = u32::MAX;
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the path carrier and both output slots are valid for this call.
        let status = unsafe {
            api.operator_exists
                .expect("WHOLE_OBJECT exists is installed")(
                operator,
                &path,
                &mut exists,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(error.is_null());
        match exists {
            0 => false,
            1 => true,
            value => panic!("exists returned noncanonical boolean {value}"),
        }
    }

    fn stat_mode_and_len(api: &ApiV1, operator: *mut OperatorV1, path: &[u8]) -> (u32, u64) {
        let path = bytes(path);
        let mut metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the path carrier and output slots are live for the duration of the call.
        let status = unsafe {
            api.operator_stat.expect("WHOLE_OBJECT stat is installed")(
                operator,
                &path,
                ptr::null(),
                &mut metadata,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!metadata.is_null());
        assert!(error.is_null());

        let absent = bytes(b"");
        let mut view = MetadataViewV1 {
            struct_size: size_of::<MetadataViewV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 0,
            mode: ENTRY_MODE_UNKNOWN,
            is_current: 0,
            is_deleted: 0,
            reserved0: 0,
            content_length: 0,
            last_modified: TimestampV1::default(),
            cache_control: absent,
            content_disposition: absent,
            content_encoding: absent,
            content_md5: absent,
            content_type: absent,
            etag: absent,
            version: absent,
        };
        // SAFETY: metadata is live and view is complete writable storage.
        unsafe {
            assert_eq!(
                api.metadata_view.expect("BASE metadata view is installed")(metadata, &mut view),
                STATUS_OK,
            );
            api.metadata_free.expect("BASE metadata free is installed")(metadata);
        }
        (view.mode, view.content_length)
    }

    #[derive(Debug, Eq, PartialEq)]
    struct ListedEntry {
        path: String,
        name: String,
        mode: u32,
        content_length: u64,
    }

    fn copy_view(view: BytesViewV1) -> Vec<u8> {
        let length = usize::try_from(view.len).expect("test view length fits usize");
        if length == 0 {
            return Vec::new();
        }
        assert!(!view.data.is_null());
        // SAFETY: the matching owned snapshot stays live while this helper copies it.
        unsafe { slice::from_raw_parts(view.data, length) }.to_vec()
    }

    fn take_listed_entry(api: &ApiV1, entry: *mut EntryV1) -> ListedEntry {
        assert!(!entry.is_null());
        let mut entry_view = EntryViewV1 {
            struct_size: size_of::<EntryViewV1>() as u32,
            struct_version: STRUCT_VERSION,
            reserved0: 0,
            path: bytes(b""),
            name: bytes(b""),
        };
        let absent = bytes(b"");
        let mut metadata_view = MetadataViewV1 {
            struct_size: size_of::<MetadataViewV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 0,
            mode: ENTRY_MODE_UNKNOWN,
            is_current: 0,
            is_deleted: 0,
            reserved0: 0,
            content_length: 0,
            last_modified: TimestampV1::default(),
            cache_control: absent,
            content_disposition: absent,
            content_encoding: absent,
            content_md5: absent,
            content_type: absent,
            etag: absent,
            version: absent,
        };
        // SAFETY: entry is live and both views are complete writable v1 storage.
        unsafe {
            assert_eq!(
                api.entry_view.expect("BASE entry view is installed")(entry, &mut entry_view),
                STATUS_OK,
            );
            assert_eq!(
                api.entry_metadata_view
                    .expect("BASE entry metadata view is installed")(
                    entry, &mut metadata_view
                ),
                STATUS_OK,
            );
        }
        let listed = ListedEntry {
            path: String::from_utf8(copy_view(entry_view.path)).expect("entry path is UTF-8"),
            name: String::from_utf8(copy_view(entry_view.name)).expect("entry name is UTF-8"),
            mode: metadata_view.mode,
            content_length: metadata_view.content_length,
        };
        // SAFETY: this test owns the entry snapshot exactly once.
        unsafe { api.entry_free.expect("BASE entry free is installed")(entry) };
        listed
    }

    fn memory_lister(
        api: &ApiV1,
        operator: *mut OperatorV1,
        path: &[u8],
        options: *const ListOptionsV1,
    ) -> *mut ListerV1 {
        let path = bytes(path);
        let mut lister = NonNull::<ListerV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: all input carriers and output slots are valid for this call.
        let status = unsafe {
            api.operator_lister
                .expect("LISTING constructor is installed")(
                operator,
                &path,
                options,
                &mut lister,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!lister.is_null());
        assert!(error.is_null());
        lister
    }

    fn memory_reader(
        api: &ApiV1,
        operator: *mut OperatorV1,
        path: &[u8],
        options: *const ReaderOptionsV1,
    ) -> *mut ReaderV1 {
        let path = bytes(path);
        let mut reader = NonNull::<ReaderV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: all input carriers and output slots are valid for this call.
        let status = unsafe {
            api.operator_reader
                .expect("RANDOM_READER constructor is installed")(
                operator,
                &path,
                options,
                &mut reader,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!reader.is_null());
        assert!(error.is_null());
        reader
    }

    fn read_stream_options(range: ByteRangeV1, chunk_size: u64) -> ReadStreamOptionsV1 {
        ReadStreamOptionsV1 {
            struct_size: size_of::<ReadStreamOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 0,
            range,
            chunk_size,
            version: bytes(b""),
            if_match: bytes(b""),
            if_none_match: bytes(b""),
        }
    }

    fn memory_read_stream(
        api: &ApiV1,
        operator: *mut OperatorV1,
        path: &[u8],
        options: &ReadStreamOptionsV1,
    ) -> *mut ReadStreamV1 {
        let path = bytes(path);
        let mut stream = NonNull::<ReadStreamV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: all input carriers and output slots are valid for this call.
        let status = unsafe {
            api.operator_read_stream
                .expect("READ_STREAM constructor is installed")(
                operator,
                &path,
                options,
                &mut stream,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!stream.is_null());
        assert!(error.is_null());
        stream
    }

    fn collect_read_stream(api: &ApiV1, stream: *mut ReadStreamV1) -> Vec<Vec<u8>> {
        let next = api.read_stream_next.expect("READ_STREAM next is installed");
        let mut chunks = Vec::new();
        loop {
            let mut buffer = NonNull::<BufferV1>::dangling().as_ptr();
            let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
            // SAFETY: stream is live and both output slots are writable.
            let status = unsafe { next(stream, MAX_OUTPUT_BYTES, &mut buffer, &mut error) };
            match status {
                STATUS_OK => {
                    assert!(!buffer.is_null());
                    assert!(error.is_null());
                    chunks.push(take_buffer(api, buffer));
                }
                STATUS_END => {
                    assert!(buffer.is_null());
                    assert!(error.is_null());
                    break;
                }
                STATUS_ERROR => {
                    assert!(buffer.is_null());
                    let kind = take_error_kind(api, error);
                    panic!("unexpected read stream error kind {kind}");
                }
                other => panic!("unexpected read stream transport status {other}"),
            }
        }
        chunks
    }

    fn memory_writer(
        api: &ApiV1,
        operator: *mut OperatorV1,
        path: &[u8],
        options: *const WriteOptionsV1,
    ) -> *mut WriterV1 {
        let path = bytes(path);
        let mut writer = NonNull::<WriterV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: all input carriers and output slots are valid for this call.
        let status = unsafe {
            api.operator_writer
                .expect("CHUNKED_WRITER constructor is installed")(
                operator,
                &path,
                options,
                &mut writer,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!writer.is_null());
        assert!(error.is_null());
        writer
    }

    fn write_writer_chunk(api: &ApiV1, writer: *mut WriterV1, data: &[u8]) {
        let data = bytes(data);
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the writer, input carrier, and error slot remain live for the call.
        let status = unsafe {
            api.writer_write.expect("CHUNKED_WRITER write is installed")(writer, &data, &mut error)
        };
        assert_eq!(status, STATUS_OK);
        assert!(error.is_null());
    }

    fn finish_writer(api: &ApiV1, writer: *mut WriterV1) -> *mut MetadataV1 {
        let mut metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the writer and both output slots remain live for the call.
        let status = unsafe {
            api.writer_close.expect("CHUNKED_WRITER close is installed")(
                writer,
                &mut metadata,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!metadata.is_null());
        assert!(error.is_null());
        metadata
    }

    fn abort_writer(api: &ApiV1, writer: *mut WriterV1) {
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the writer and optional error slot remain live for this call.
        let status = unsafe {
            api.writer_abort
                .expect("WRITER_ABORT operation is installed")(writer, &mut error)
        };
        assert_eq!(status, STATUS_OK);
        assert!(error.is_null());
    }

    fn take_metadata_content_length(api: &ApiV1, metadata: *mut MetadataV1) -> u64 {
        let absent = bytes(b"");
        let mut view = MetadataViewV1 {
            struct_size: size_of::<MetadataViewV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 0,
            mode: ENTRY_MODE_UNKNOWN,
            is_current: 0,
            is_deleted: 0,
            reserved0: 0,
            content_length: 0,
            last_modified: TimestampV1::default(),
            cache_control: absent,
            content_disposition: absent,
            content_encoding: absent,
            content_md5: absent,
            content_type: absent,
            etag: absent,
            version: absent,
        };
        // SAFETY: metadata is live and view is complete writable storage.
        unsafe {
            assert_eq!(
                api.metadata_view.expect("BASE metadata view is installed")(metadata, &mut view),
                STATUS_OK,
            );
            api.metadata_free.expect("BASE metadata free is installed")(metadata);
        }
        view.content_length
    }

    fn read_object(api: &ApiV1, operator: *mut OperatorV1, path: &[u8]) -> Vec<u8> {
        let path = bytes(path);
        let mut buffer = NonNull::<BufferV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the operator, path, and output slots remain live for the call.
        let status = unsafe {
            api.operator_read.expect("WHOLE_OBJECT read is installed")(
                operator,
                &path,
                ptr::null(),
                MAX_OUTPUT_BYTES,
                &mut buffer,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!buffer.is_null());
        assert!(error.is_null());
        take_buffer(api, buffer)
    }

    fn byte_range(kind: u32, offset: u64, length: u64) -> ByteRangeV1 {
        ByteRangeV1 {
            struct_size: size_of::<ByteRangeV1>() as u32,
            struct_version: STRUCT_VERSION,
            kind,
            reserved0: 0,
            offset,
            length,
        }
    }

    fn whole_read_options(range: ByteRangeV1) -> ReadOptionsV1 {
        ReadOptionsV1 {
            struct_size: size_of::<ReadOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 0,
            range,
            version: bytes(b""),
            if_match: bytes(b""),
            if_none_match: bytes(b""),
        }
    }

    fn try_read_object(
        api: &ApiV1,
        operator: *mut OperatorV1,
        path: &[u8],
        options: *const ReadOptionsV1,
        max_output_len: u64,
    ) -> Result<Vec<u8>, u32> {
        let path = bytes(path);
        let mut buffer = NonNull::<BufferV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the operator, path, optional options, and outputs remain live for the call.
        let status = unsafe {
            api.operator_read.expect("WHOLE_OBJECT read is installed")(
                operator,
                &path,
                options,
                max_output_len,
                &mut buffer,
                &mut error,
            )
        };
        match status {
            STATUS_OK => {
                assert!(!buffer.is_null());
                assert!(error.is_null());
                Ok(take_buffer(api, buffer))
            }
            STATUS_ERROR => {
                assert!(buffer.is_null());
                assert!(!error.is_null());
                Err(take_error_kind(api, error))
            }
            other => panic!("whole read returned unexpected status {other}"),
        }
    }

    fn take_buffer(api: &ApiV1, buffer: *mut BufferV1) -> Vec<u8> {
        assert!(!buffer.is_null());
        let mut length = u64::MAX;
        // SAFETY: the buffer is live and length is writable.
        assert_eq!(
            unsafe { api.buffer_len.expect("BASE buffer len is installed")(buffer, &mut length) },
            STATUS_OK,
        );
        let length = usize::try_from(length).expect("test buffer length fits usize");
        let mut output = vec![0; length];
        let mut required = u64::MAX;
        let destination = if output.is_empty() {
            ptr::null_mut()
        } else {
            output.as_mut_ptr()
        };
        // SAFETY: destination covers the reported immutable buffer length.
        unsafe {
            assert_eq!(
                api.buffer_copy.expect("BASE buffer copy is installed")(
                    buffer,
                    destination,
                    length as u64,
                    &mut required,
                ),
                STATUS_OK,
            );
            api.buffer_free.expect("BASE buffer free is installed")(buffer);
        }
        assert_eq!(required, length as u64);
        output
    }

    fn read_reader_bytes(
        api: &ApiV1,
        reader: *mut ReaderV1,
        range: &ByteRangeV1,
        max_output_len: u64,
    ) -> Vec<u8> {
        let mut buffer = NonNull::<BufferV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the reader, range, and output slots remain live for this call.
        let status = unsafe {
            api.reader_read.expect("RANDOM_READER read is installed")(
                reader,
                range,
                max_output_len,
                &mut buffer,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_OK);
        assert!(!buffer.is_null());
        assert!(error.is_null());
        take_buffer(api, buffer)
    }

    fn collect_lister(api: &ApiV1, lister: *mut ListerV1) -> Vec<ListedEntry> {
        let next = api.lister_next.expect("LISTING next is installed");
        let mut entries = Vec::new();
        loop {
            let mut entry = NonNull::<EntryV1>::dangling().as_ptr();
            let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
            // SAFETY: the lister is live and both output slots are writable.
            let status = unsafe { next(lister, &mut entry, &mut error) };
            match status {
                STATUS_OK => {
                    assert!(!entry.is_null());
                    assert!(error.is_null());
                    entries.push(take_listed_entry(api, entry));
                }
                STATUS_END => {
                    assert!(entry.is_null());
                    assert!(error.is_null());
                    break;
                }
                STATUS_ERROR => {
                    assert!(entry.is_null());
                    let kind = take_error_kind(api, error);
                    panic!("unexpected lister error kind {kind}");
                }
                other => panic!("unexpected lister transport status {other}"),
            }
        }
        entries
    }

    #[test]
    fn library_info_reports_the_selected_service_profile() {
        let expected_profile = if cfg!(feature = "profile-standard") {
            "standard"
        } else {
            "local"
        };
        assert_eq!(SERVICE_PROFILE, expected_profile);
        let mut view = LibraryInfoViewV1 {
            struct_size: size_of::<LibraryInfoViewV1>() as u32,
            struct_version: STRUCT_VERSION,
            reserved0: 0,
            binding_version: bytes(b""),
            opendal_version: bytes(b""),
            service_profile: bytes(b""),
        };
        // SAFETY: the complete output carrier is writable for this call.
        assert_eq!(unsafe { library_info(&mut view) }, STATUS_OK);
        assert_eq!(copy_view(view.binding_version), BINDING_VERSION.as_bytes());
        assert_eq!(copy_view(view.opendal_version), OPENDAL_VERSION.as_bytes());
        assert_eq!(copy_view(view.service_profile), SERVICE_PROFILE.as_bytes());
    }

    #[test]
    fn bootstrap_installs_complete_supported_groups() {
        let api = api();
        assert_eq!(api.library_struct_size as usize, size_of::<ApiV1>());
        assert_eq!(
            api.feature_bits,
            FEATURE_BASE
                | FEATURE_WHOLE_OBJECT
                | FEATURE_LISTING
                | FEATURE_RANDOM_READER
                | FEATURE_CHUNKED_WRITER
                | FEATURE_READ_STREAM
                | FEATURE_WRITER_ABORT
                | FEATURE_PRESIGN
                | PROFILE_FEATURE_BITS
                | LAYER_FEATURE_BITS,
        );
        assert!(api.library_info.is_some());
        assert!(api.operator_new.is_some());
        assert!(api.operator_rename.is_some());
        assert!(api.operator_lister.is_some());
        assert!(api.lister_next.is_some());
        assert!(api.lister_close.is_some());
        assert!(api.lister_free.is_some());
        assert!(api.operator_reader.is_some());
        assert!(api.reader_read.is_some());
        assert!(api.reader_close.is_some());
        assert!(api.reader_free.is_some());
        assert!(api.operator_writer.is_some());
        assert!(api.writer_write.is_some());
        assert!(api.writer_close.is_some());
        assert!(api.writer_free.is_some());
        assert!(api.operator_read_stream.is_some());
        assert!(api.read_stream_next.is_some());
        assert!(api.read_stream_close.is_some());
        assert!(api.read_stream_free.is_some());
        assert!(api.writer_abort.is_some());
        #[cfg(feature = "profile-standard")]
        assert!(api.operator_s3.is_some());
        #[cfg(not(feature = "profile-standard"))]
        assert!(api.operator_s3.is_none());
        assert!(api.operator_presign_read.is_some());
        assert!(api.operator_presign_write.is_some());
        assert!(api.operator_presign_stat.is_some());
        assert!(api.presigned_request_view.is_some());
        assert!(api.presigned_request_header_view.is_some());
        assert!(api.presigned_request_free.is_some());
        #[cfg(feature = "layers-timeout-retry")]
        {
            assert!(api.operator_with_timeout.is_some());
            assert!(api.operator_with_retry.is_some());
        }
        #[cfg(not(feature = "layers-timeout-retry"))]
        {
            assert!(api.operator_with_timeout.is_none());
            assert!(api.operator_with_retry.is_none());
        }
    }

    #[cfg(feature = "profile-standard")]
    #[test]
    fn s3_authentication_modes_construct_without_io() {
        let api = api();

        let mut default_chain = s3_options();
        default_chain.auth_kind = S3_AUTH_DEFAULT_CHAIN;
        default_chain.flags = S3_DISABLE_EC2_METADATA;
        assert_s3_constructs(&api, &default_chain);

        let mut static_credentials = s3_options();
        static_credentials.auth_kind = S3_AUTH_STATIC;
        static_credentials.source_kind = S3_SOURCE_STATIC;
        static_credentials.present_bits |= S3_SESSION_TOKEN_PRESENT;
        static_credentials.access_key_id = bytes(b"test-access-key");
        static_credentials.secret_access_key = bytes(b"test-secret-key");
        static_credentials.session_token = bytes(b"test-session-token");
        assert_s3_constructs(&api, &static_credentials);

        assert_s3_constructs(&api, &s3_options());

        let mut assume_role = s3_options();
        assume_role.auth_kind = S3_AUTH_ASSUME_ROLE;
        assume_role.source_kind = S3_SOURCE_STATIC;
        assume_role.present_bits |= S3_SESSION_TOKEN_PRESENT
            | S3_EXTERNAL_ID_PRESENT
            | S3_ROLE_SESSION_NAME_PRESENT
            | S3_ASSUME_ROLE_DURATION_PRESENT;
        assume_role.access_key_id = bytes(b"source-access-key");
        assume_role.secret_access_key = bytes(b"source-secret-key");
        assume_role.session_token = bytes(b"source-session-token");
        assume_role.role_arn = bytes(b"arn:aws:iam::123456789012:role/moonbit-test");
        assume_role.external_id = bytes(b"moonbit-external");
        assume_role.role_session_name = bytes(b"moonbit-session");
        assume_role.assume_role_duration_seconds = 900;
        assert_s3_constructs(&api, &assume_role);
    }

    #[cfg(feature = "profile-standard")]
    #[test]
    fn unsigned_s3_presign_roundtrips_without_storage_io() {
        let api = api();
        let options = s3_options();
        let mut operator = ptr::null_mut();
        let mut info = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: the complete carrier and all output slots stay live.
        assert_eq!(
            unsafe {
                api.operator_s3.expect("S3 constructor is installed")(
                    &options,
                    &mut operator,
                    &mut info,
                    &mut error,
                )
            },
            STATUS_OK,
        );
        assert!(!operator.is_null());
        assert!(!info.is_null());
        assert!(error.is_null());

        let path = bytes(b"folder/object.txt");
        let mut request = ptr::null_mut();
        // SAFETY: the operator, path, and outputs remain live for the call.
        assert_eq!(
            unsafe {
                api.operator_presign_read
                    .expect("PRESIGN read is installed")(
                    operator,
                    &path,
                    ptr::null(),
                    60,
                    &mut request,
                    &mut error,
                )
            },
            STATUS_OK,
        );
        assert!(!request.is_null());
        assert!(error.is_null());
        let mut view = PresignedRequestViewV1 {
            struct_size: size_of::<PresignedRequestViewV1>() as u32,
            struct_version: STRUCT_VERSION,
            reserved0: 0,
            method: bytes(b""),
            uri: bytes(b""),
            header_count: 0,
        };
        // SAFETY: request remains live while borrowed views are copied.
        assert_eq!(
            unsafe {
                api.presigned_request_view
                    .expect("PRESIGN request view is installed")(request, &mut view)
            },
            STATUS_OK,
        );
        assert_eq!(copy_view(view.method), b"GET");
        let uri = String::from_utf8(copy_view(view.uri)).expect("presigned URI is UTF-8");
        assert!(uri.contains("moonbit-binding-test"));
        assert!(uri.contains("folder/object.txt"));
        // SAFETY: this test owns all returned handles exactly once.
        unsafe {
            api.presigned_request_free
                .expect("PRESIGN free is installed")(request);
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[cfg(feature = "profile-standard")]
    #[test]
    fn s3_rejects_unknown_partial_conflicting_and_noncanonical_inputs() {
        let api = api();
        let constructor = api.operator_s3.expect("S3 constructor is installed");

        let mut cases = Vec::new();
        let mut unknown = s3_options();
        unknown.flags = 1 << 63;
        cases.push((unknown, STATUS_ABI_MISMATCH, None));

        let mut partial_static = s3_options();
        partial_static.auth_kind = S3_AUTH_STATIC;
        partial_static.source_kind = S3_SOURCE_STATIC;
        partial_static.access_key_id = bytes(b"only-one-half");
        cases.push((partial_static, STATUS_ERROR, Some(ERROR_INVALID_ARGUMENT)));

        let mut conflicting = s3_options();
        conflicting.auth_kind = S3_AUTH_DEFAULT_CHAIN;
        conflicting.source_kind = S3_SOURCE_STATIC;
        cases.push((conflicting, STATUS_ERROR, Some(ERROR_INVALID_ARGUMENT)));

        let mut noncanonical = s3_options();
        noncanonical.root = bytes(b"hidden-without-presence-bit");
        cases.push((noncanonical, STATUS_ABI_MISMATCH, None));

        let mut empty_region = s3_options();
        empty_region.region = bytes(b"");
        cases.push((empty_region, STATUS_ERROR, Some(ERROR_INVALID_ARGUMENT)));

        for (options, expected_status, expected_kind) in cases {
            let mut operator = NonNull::<OperatorV1>::dangling().as_ptr();
            let mut info = NonNull::<OperatorInfoV1>::dangling().as_ptr();
            let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
            // SAFETY: carriers and output slots remain live for each call.
            let status = unsafe { constructor(&options, &mut operator, &mut info, &mut error) };
            assert_eq!(status, expected_status);
            assert!(operator.is_null());
            assert!(info.is_null());
            match expected_kind {
                Some(kind) => assert_eq!(take_error_kind(&api, error), kind),
                None => assert!(error.is_null()),
            }
        }
    }

    #[cfg(feature = "profile-standard")]
    #[test]
    fn s3_virtual_host_dotted_bucket_redacts_construction_secrets() {
        let api = api();
        let secret = b"never-include-this-secret";
        let mut options = s3_options();
        options.bucket = bytes(b"dotted.bucket");
        options.flags = S3_VIRTUAL_HOST_STYLE;
        options.auth_kind = S3_AUTH_STATIC;
        options.source_kind = S3_SOURCE_STATIC;
        options.access_key_id = bytes(b"test-access-key");
        options.secret_access_key = bytes(secret);

        let mut operator = NonNull::<OperatorV1>::dangling().as_ptr();
        let mut info = NonNull::<OperatorInfoV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the complete carrier and all outputs remain live for the call.
        let status = unsafe {
            api.operator_s3.expect("S3 constructor is installed")(
                &options,
                &mut operator,
                &mut info,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_ERROR);
        assert!(operator.is_null());
        assert!(info.is_null());
        assert!(!error.is_null());
        // SAFETY: the error handle is owned until it is freed below.
        let snapshot = unsafe { &*error };
        assert_eq!(snapshot.kind, ERROR_CONFIG_INVALID);
        assert_eq!(snapshot.message, "operator construction failed");
        assert!(
            !snapshot
                .message
                .as_bytes()
                .windows(secret.len())
                .any(|v| v == secret)
        );
        // SAFETY: this test owns the error handle exactly once.
        unsafe { api.error_free.expect("BASE error free is installed")(error) };
    }

    #[cfg(feature = "layers-timeout-retry")]
    #[test]
    fn immutable_layers_rebuild_both_operator_facades_in_call_order() {
        let api = api();
        let (base, info) = memory_operator(&api);
        write_memory_object(&api, base, b"before-layer", b"base");

        let timeout = timeout_operator(&api, base, 5_000, 2_000).expect("timeout layer succeeds");
        let retry = retry_operator(&api, timeout, 3, 1, 5, 0).expect("retry layer succeeds");

        // SAFETY: all three independently owned handles remain live.
        unsafe {
            assert_eq!((*base).layer_bits, 0);
            assert_eq!((*timeout).layer_bits, OPERATOR_LAYER_TIMEOUT);
            assert_eq!(
                (*retry).layer_bits,
                OPERATOR_LAYER_TIMEOUT | OPERATOR_LAYER_RETRY,
            );
        }
        assert_eq!(read_object(&api, retry, b"before-layer"), b"base");
        write_memory_object(&api, retry, b"after-layer", b"layered");
        assert_eq!(read_object(&api, base, b"after-layer"), b"layered");

        // The async and blocking facets are rebuilt from the same layered
        // Operator, so both continue to see the same backend and layer program.
        // SAFETY: retry is live and cloning the async facet does not borrow it.
        let async_operator = unsafe { (*retry).async_inner.clone() };
        let runtime = runtime().unwrap_or_else(|_| panic!("test runtime must initialize"));
        let async_bytes = runtime
            .block_on(async_operator.read("after-layer"))
            .expect("layered async read succeeds");
        assert_eq!(async_bytes.to_vec(), b"layered");

        // SAFETY: every handle is independently owned and freed exactly once.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(base);
            api.operator_free.expect("BASE operator free is installed")(timeout);
            api.operator_free.expect("BASE operator free is installed")(retry);
        }
    }

    #[cfg(feature = "layers-timeout-retry")]
    #[test]
    fn layer_validation_rejects_zero_bounds_duplicates_and_reverse_order() {
        let api = api();
        let (base, info) = memory_operator(&api);

        for result in [
            timeout_operator(&api, base, 0, 1),
            timeout_operator(&api, base, 1, 0),
        ] {
            assert_eq!(result, Err(ERROR_INVALID_ARGUMENT));
        }
        for result in [
            retry_operator(&api, base, 0, 1, 1, 0),
            retry_operator(&api, base, 1, 0, 1, 0),
            retry_operator(&api, base, 1, 1, 0, 0),
            retry_operator(&api, base, 1, 2, 1, 0),
        ] {
            assert_eq!(result, Err(ERROR_INVALID_ARGUMENT));
        }
        assert_eq!(
            retry_operator(&api, base, 1, 1, 1, 2),
            Err(STATUS_ABI_MISMATCH),
        );

        let timeout = timeout_operator(&api, base, 1, 1).expect("first timeout succeeds");
        assert_eq!(
            timeout_operator(&api, timeout, 1, 1),
            Err(ERROR_INVALID_ARGUMENT),
        );
        let retry = retry_operator(&api, timeout, 1, 1, 1, 1).expect("first retry succeeds");
        assert_eq!(
            retry_operator(&api, retry, 1, 1, 1, 0),
            Err(ERROR_INVALID_ARGUMENT),
        );
        assert_eq!(
            timeout_operator(&api, retry, 1, 1),
            Err(ERROR_INVALID_ARGUMENT),
        );

        // A retry-only Operator is allowed, but timeout cannot later be placed
        // outside it because that would change the documented attempt budget.
        let retry_only = retry_operator(&api, base, 1, 1, 1, 0).expect("retry succeeds");
        assert_eq!(
            timeout_operator(&api, retry_only, 1, 1),
            Err(ERROR_INVALID_ARGUMENT),
        );

        // SAFETY: every handle is independently owned and freed exactly once.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            for operator in [base, timeout, retry, retry_only] {
                api.operator_free.expect("BASE operator free is installed")(operator);
            }
        }
    }

    #[test]
    fn bootstrap_never_writes_a_cut_function_or_caller_tail() {
        #[repr(C, align(16))]
        struct Storage([u8; size_of::<ApiV1>() + 16]);

        let field_start = offset_of!(ApiV1, operator_new);
        let cut = field_start + size_of::<Option<OperatorNewFn>>() / 2;
        let mut storage = Storage([0xA5; size_of::<ApiV1>() + 16]);
        storage.0[..cut].fill(0);
        storage.0[..4].copy_from_slice(&u32::try_from(cut).expect("cut fits u32").to_ne_bytes());
        storage.0[4..8].copy_from_slice(&ABI_MAJOR.to_ne_bytes());

        // SAFETY: aligned storage covers the declared prefix and permanent input.
        let status = unsafe { opendal_mbt_get_api(storage.0.as_mut_ptr().cast()) };
        assert_eq!(status, STATUS_OK);
        assert!(storage.0[field_start..cut].iter().all(|byte| *byte == 0));
        assert!(storage.0[cut..].iter().all(|byte| *byte == 0xA5));
    }

    #[test]
    fn bootstrap_respects_every_table_boundary_and_long_caller_tail() {
        #[repr(C, align(16))]
        struct Storage([u8; size_of::<ApiV1>() + 16]);

        let staged = stage_api().expect("the test target can stage the v1 API");
        macro_rules! field_bounds {
            ($($field:ident),+ $(,)?) => {
                [$(
                    (
                        offset_of!(ApiV1, $field),
                        size_of_val(&staged.$field),
                    ),
                )+]
            };
        }
        let fields = field_bounds!(
            library_struct_size,
            library_minor,
            library_patch,
            reserved0,
            feature_bits,
            max_output_bytes,
            library_info,
            error_view,
            error_free,
            buffer_len,
            buffer_copy,
            buffer_free,
            metadata_view,
            metadata_free,
            entry_view,
            entry_metadata_view,
            entry_free,
            operator_info_view,
            operator_info_free,
            operator_new,
            operator_free,
            operator_check,
            operator_exists,
            operator_stat,
            operator_read,
            operator_write,
            operator_create_dir,
            operator_delete,
            operator_copy,
            operator_rename,
            operator_lister,
            lister_next,
            lister_close,
            lister_free,
            operator_reader,
            reader_read,
            reader_close,
            reader_free,
            operator_writer,
            writer_write,
            writer_close,
            writer_free,
            operator_read_stream,
            read_stream_next,
            read_stream_close,
            read_stream_free,
            writer_abort,
            operator_s3,
            operator_presign_read,
            operator_presign_write,
            operator_presign_stat,
            presigned_request_view,
            presigned_request_header_view,
            presigned_request_free,
            operator_with_timeout,
            operator_with_retry,
        );

        for caller_size in API_PREFIX_SIZE..=size_of::<ApiV1>() + 16 {
            let mut storage = Storage([0xA5; size_of::<ApiV1>() + 16]);
            storage.0[..4].copy_from_slice(
                &u32::try_from(caller_size)
                    .expect("test caller size fits u32")
                    .to_ne_bytes(),
            );
            storage.0[4..8].copy_from_slice(&ABI_MAJOR.to_ne_bytes());

            // SAFETY: aligned storage covers the caller-declared size.
            assert_eq!(
                unsafe { opendal_mbt_get_api(storage.0.as_mut_ptr().cast()) },
                STATUS_OK,
                "caller_size={caller_size}",
            );

            let installed = caller_size.min(size_of::<ApiV1>());
            assert!(
                storage.0[installed..].iter().all(|byte| *byte == 0xA5),
                "caller tail changed at caller_size={caller_size}",
            );
            for (start, size) in fields {
                let end = start + size;
                if start < installed && installed < end {
                    assert!(
                        storage.0[start..installed].iter().all(|byte| *byte == 0),
                        "partially covered field changed at caller_size={caller_size}",
                    );
                }
            }
        }

        let mut short = Storage([0xA5; size_of::<ApiV1>() + 16]);
        let caller_size = API_PREFIX_SIZE - 1;
        short.0[..4].copy_from_slice(&(caller_size as u32).to_ne_bytes());
        short.0[4..8].copy_from_slice(&ABI_MAJOR.to_ne_bytes());
        let preserved = short.0[API_INPUT_SIZE..].to_vec();
        // SAFETY: the bootstrap input prefix itself is readable and aligned.
        assert_eq!(
            unsafe { opendal_mbt_get_api(short.0.as_mut_ptr().cast()) },
            STATUS_ABI_MISMATCH,
        );
        assert_eq!(&short.0[API_INPUT_SIZE..], preserved);
    }

    #[test]
    fn unsupported_bootstrap_preserves_payload() {
        let mut storage = MaybeUninit::<ApiV1>::zeroed();
        // SAFETY: write and read only fields covered by full aligned storage.
        unsafe {
            let pointer = storage.as_mut_ptr();
            ptr::addr_of_mut!((*pointer).struct_size)
                .write(u32::try_from(size_of::<ApiV1>()).expect("table fits u32"));
            ptr::addr_of_mut!((*pointer).requested_major).write(99);
            ptr::addr_of_mut!((*pointer).library_patch).write(0xDEAD_BEEF);
            assert_eq!(opendal_mbt_get_api(pointer.cast()), STATUS_ABI_MISMATCH);
            assert_eq!(ptr::addr_of!((*pointer).library_patch).read(), 0xDEAD_BEEF);
        }
    }

    #[test]
    fn memory_binary_roundtrip_missing_error_and_null_frees() {
        let api = api();
        let new = api.operator_new.expect("BASE constructor is installed");
        let write = api.operator_write.expect("WHOLE_OBJECT write is installed");
        let read = api.operator_read.expect("WHOLE_OBJECT read is installed");
        let error_view = api.error_view.expect("BASE error view is installed");
        let buffer_copy = api.buffer_copy.expect("BASE buffer copy is installed");
        let scheme = bytes(b"memory");
        let mut operator = ptr::null_mut();
        let mut info = ptr::null_mut();
        let mut error = ptr::null_mut();

        // SAFETY: all pointers and views obey the C ABI contract.
        unsafe {
            assert_eq!(
                new(
                    &scheme,
                    ptr::null(),
                    0,
                    &mut operator,
                    &mut info,
                    &mut error,
                ),
                STATUS_OK
            );
            assert!(!operator.is_null());
            assert!(!info.is_null());
            assert!(error.is_null());

            let path = bytes(b"binary/data");
            let payload = b"\0moonbit\xFFopendal\0";
            let data = bytes(payload);
            let mut metadata = ptr::null_mut();
            assert_eq!(
                write(
                    operator,
                    &path,
                    &data,
                    ptr::null(),
                    &mut metadata,
                    &mut error,
                ),
                STATUS_OK
            );
            (api.metadata_free.expect("metadata free installed"))(metadata);

            let mut buffer = ptr::null_mut();
            assert_eq!(
                read(
                    operator,
                    &path,
                    ptr::null(),
                    MAX_OUTPUT_BYTES,
                    &mut buffer,
                    &mut error,
                ),
                STATUS_OK
            );
            let mut required = 0;
            assert_eq!(
                buffer_copy(buffer, ptr::null_mut(), 0, &mut required),
                STATUS_BUFFER_TOO_SMALL
            );
            let mut output = vec![0xCC; required as usize + 3];
            assert_eq!(
                buffer_copy(
                    buffer,
                    output.as_mut_ptr(),
                    output.len() as u64,
                    &mut required,
                ),
                STATUS_OK
            );
            assert_eq!(&output[..payload.len()], payload);
            assert_eq!(&output[payload.len()..], &[0xCC; 3]);
            (api.buffer_free.expect("buffer free installed"))(buffer);

            assert_eq!(
                read(operator, &path, ptr::null(), 4, &mut buffer, &mut error,),
                STATUS_ERROR
            );
            assert!(buffer.is_null());
            assert!(!error.is_null());
            let mut limited_view = ErrorViewV1 {
                struct_size: size_of::<ErrorViewV1>() as u32,
                struct_version: STRUCT_VERSION,
                kind: 0,
                status: 0,
                kind_name: bytes(b""),
                message: bytes(b""),
            };
            assert_eq!(error_view(error, &mut limited_view), STATUS_OK);
            assert_eq!(limited_view.kind, ERROR_BUFFER_TOO_LARGE);
            (api.error_free.expect("error free installed"))(error);

            let missing = bytes(b"does-not-exist");
            buffer = ptr::null_mut();
            assert_eq!(
                read(
                    operator,
                    &missing,
                    ptr::null(),
                    MAX_OUTPUT_BYTES,
                    &mut buffer,
                    &mut error,
                ),
                STATUS_ERROR
            );
            assert!(buffer.is_null());
            assert!(!error.is_null());
            let mut view = ErrorViewV1 {
                struct_size: size_of::<ErrorViewV1>() as u32,
                struct_version: STRUCT_VERSION,
                kind: 0,
                status: 0,
                kind_name: bytes(b""),
                message: bytes(b""),
            };
            assert_eq!(error_view(error, &mut view), STATUS_OK);
            assert_eq!(view.kind, ERROR_NOT_FOUND);
            (api.error_free.expect("error free installed"))(error);

            (api.error_free.expect("error free installed"))(ptr::null_mut());
            (api.buffer_free.expect("buffer free installed"))(ptr::null_mut());
            (api.metadata_free.expect("metadata free installed"))(ptr::null_mut());
            (api.entry_free.expect("entry free installed"))(ptr::null_mut());
            (api.operator_info_free.expect("info free installed"))(info);
            (api.operator_info_free.expect("info free installed"))(ptr::null_mut());
            (api.operator_free.expect("operator free installed"))(operator);
            (api.operator_free.expect("operator free installed"))(ptr::null_mut());
        }
    }

    #[test]
    fn whole_read_uses_only_the_negotiated_overflow_probe() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let payload = vec![0xA5; WHOLE_READ_CHUNK_BYTES * 2 + 17];
        write_memory_object(&api, operator, b"bounded/large.bin", &payload);

        install_whole_read_observer(operator);
        assert_eq!(
            try_read_object(&api, operator, b"bounded/large.bin", ptr::null(), 7,),
            Err(ERROR_BUFFER_TOO_LARGE),
        );
        assert_eq!(take_whole_read_observation(operator), 8);

        // SAFETY: both constructor outputs remain uniquely owned by this test.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn whole_read_handles_empty_zero_and_exact_limits() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        write_memory_object(&api, operator, b"bounded/empty", b"");
        write_memory_object(&api, operator, b"bounded/exact", b"12345678");

        install_whole_read_observer(operator);
        assert_eq!(
            try_read_object(&api, operator, b"bounded/empty", ptr::null(), 0),
            Ok(Vec::new()),
        );
        assert_eq!(take_whole_read_observation(operator), 0);

        install_whole_read_observer(operator);
        assert_eq!(
            try_read_object(&api, operator, b"bounded/exact", ptr::null(), 8),
            Ok(b"12345678".to_vec()),
        );
        assert_eq!(take_whole_read_observation(operator), 8);

        install_whole_read_observer(operator);
        assert_eq!(
            try_read_object(&api, operator, b"bounded/exact", ptr::null(), 0),
            Err(ERROR_BUFFER_TOO_LARGE),
        );
        assert_eq!(take_whole_read_observation(operator), 1);

        // SAFETY: both constructor outputs remain uniquely owned by this test.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn whole_read_preserves_ranges_and_preflights_known_lengths() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        write_memory_object(&api, operator, b"bounded/ranges", b"0123456789");

        let from = whole_read_options(byte_range(RANGE_FROM, 3, 0));
        install_whole_read_observer(operator);
        assert_eq!(
            try_read_object(&api, operator, b"bounded/ranges", &from, 7),
            Ok(b"3456789".to_vec()),
        );
        assert_eq!(take_whole_read_observation(operator), 7);

        let range = whole_read_options(byte_range(RANGE_OFFSET_LENGTH, 2, 4));
        install_whole_read_observer(operator);
        assert_eq!(
            try_read_object(&api, operator, b"bounded/ranges", &range, 4),
            Ok(b"2345".to_vec()),
        );
        assert_eq!(take_whole_read_observation(operator), 4);

        let suffix = whole_read_options(byte_range(RANGE_SUFFIX, 0, 3));
        install_whole_read_observer(operator);
        assert_eq!(
            try_read_object(&api, operator, b"bounded/ranges", &suffix, 3),
            Ok(b"789".to_vec()),
        );
        assert_eq!(take_whole_read_observation(operator), 3);

        let oversized_range = whole_read_options(byte_range(RANGE_OFFSET_LENGTH, 2, 6));
        install_whole_read_observer(operator);
        assert_eq!(
            try_read_object(&api, operator, b"bounded/ranges", &oversized_range, 5,),
            Err(ERROR_BUFFER_TOO_LARGE),
        );
        assert_eq!(take_whole_read_observation(operator), 0);

        let oversized_suffix = whole_read_options(byte_range(RANGE_SUFFIX, 0, 6));
        install_whole_read_observer(operator);
        assert_eq!(
            try_read_object(&api, operator, b"bounded/ranges", &oversized_suffix, 5,),
            Err(ERROR_BUFFER_TOO_LARGE),
        );
        assert_eq!(take_whole_read_observation(operator), 0);

        // SAFETY: both constructor outputs remain uniquely owned by this test.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn whole_read_plan_preserves_conditions_and_forces_serial_backpressure() {
        let options = ReadOptions {
            range: BytesRange::suffix(3),
            version: Some("version-1".to_owned()),
            if_match: Some("etag-1".to_owned()),
            if_none_match: Some("etag-2".to_owned()),
            content_length_hint: Some(10),
            ..ReadOptions::default()
        };
        let plan = match whole_read_plan(options, 3) {
            Ok(plan) => plan,
            Err(_) => panic!("valid bounded whole-read plan was rejected"),
        };

        assert_eq!(plan.range, WholeReadRangePlan::Suffix { size: 3 });
        assert_eq!(plan.output_limit, 3);
        assert_eq!(plan.reader_options.version.as_deref(), Some("version-1"));
        assert_eq!(plan.reader_options.if_match.as_deref(), Some("etag-1"));
        assert_eq!(plan.reader_options.if_none_match.as_deref(), Some("etag-2"));
        assert_eq!(plan.reader_options.content_length_hint, Some(10));
        assert_eq!(plan.reader_options.concurrent, 1);
        assert_eq!(plan.reader_options.chunk, Some(WHOLE_READ_CHUNK_BYTES));
        assert_eq!(plan.reader_options.prefetch, 0);
    }

    #[test]
    fn base_inputs_reject_duplicate_config_and_preserve_atomic_outputs() {
        let api = api();
        let new = api.operator_new.expect("BASE constructor is installed");
        let root = bytes(b"root");
        let root_upper = bytes(b"ROOT");
        let value = bytes(b"/");
        let config = [
            KvV1 { key: root, value },
            KvV1 {
                key: root_upper,
                value,
            },
        ];
        let scheme = bytes(b"memory");
        let mut operator = NonNull::<OperatorV1>::dangling().as_ptr();
        let mut info = NonNull::<OperatorInfoV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();

        // SAFETY: all input regions and output slots are valid for the call.
        let status = unsafe {
            new(
                &scheme,
                config.as_ptr(),
                config.len() as u64,
                &mut operator,
                &mut info,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_ERROR);
        assert!(operator.is_null());
        assert!(info.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_INVALID_ARGUMENT);

        operator = NonNull::<OperatorV1>::dangling().as_ptr();
        info = NonNull::<OperatorInfoV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: required outputs are writable; NULL with a nonzero config length
        // is deliberately malformed and must fail before backend construction.
        let status = unsafe {
            new(
                &scheme,
                ptr::null(),
                1,
                &mut operator,
                &mut info,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_ABI_MISMATCH);
        assert!(operator.is_null());
        assert!(info.is_null());
        assert!(error.is_null());

        info = NonNull::<OperatorInfoV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the first required output is deliberately NULL. The other
        // valid outputs must still be cleared before the ABI mismatch returns.
        let status = unsafe {
            new(
                &scheme,
                ptr::null(),
                0,
                ptr::null_mut(),
                &mut info,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_ABI_MISMATCH);
        assert!(info.is_null());
        assert!(error.is_null());
    }

    #[test]
    fn whole_object_options_fail_atomically_at_the_abi_boundary() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let read = api.operator_read.expect("WHOLE_OBJECT read is installed");
        let write = api.operator_write.expect("WHOLE_OBJECT write is installed");
        let path = bytes(b"options-boundary");
        let absent = bytes(b"");
        let mut read_options = ReadOptionsV1 {
            struct_size: size_of::<ReadOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 1 << 63,
            range: ByteRangeV1 {
                struct_size: size_of::<ByteRangeV1>() as u32,
                struct_version: STRUCT_VERSION,
                kind: RANGE_FULL,
                reserved0: 0,
                offset: 0,
                length: 0,
            },
            version: absent,
            if_match: absent,
            if_none_match: absent,
        };
        let mut buffer = NonNull::<BufferV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();

        // SAFETY: outputs are writable and malformed options are fully readable.
        assert_eq!(
            unsafe {
                read(
                    operator,
                    &path,
                    &read_options,
                    MAX_OUTPUT_BYTES,
                    &mut buffer,
                    &mut error,
                )
            },
            STATUS_ABI_MISMATCH,
        );
        assert!(buffer.is_null());
        assert!(error.is_null());

        read_options.present_bits = 0;
        read_options.version = bytes(b"noncanonical-absent");
        buffer = NonNull::<BufferV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: same valid carriers, with a deliberately noncanonical absent value.
        assert_eq!(
            unsafe {
                read(
                    operator,
                    &path,
                    &read_options,
                    MAX_OUTPUT_BYTES,
                    &mut buffer,
                    &mut error,
                )
            },
            STATUS_ABI_MISMATCH,
        );
        assert!(buffer.is_null());
        assert!(error.is_null());

        let mut write_options = WriteOptionsV1 {
            struct_size: size_of::<WriteOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 0,
            flags: 1 << 63,
            content_type: absent,
            content_disposition: absent,
            content_encoding: absent,
            cache_control: absent,
            if_match: absent,
            if_none_match: absent,
        };
        let payload = bytes(b"must-not-be-written");
        let mut metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: outputs and carriers are valid; the unknown flag is intentional.
        assert_eq!(
            unsafe {
                write(
                    operator,
                    &path,
                    &payload,
                    &write_options,
                    &mut metadata,
                    &mut error,
                )
            },
            STATUS_ABI_MISMATCH,
        );
        assert!(metadata.is_null());
        assert!(error.is_null());

        write_options.flags = 0;
        write_options.struct_size = (size_of::<WriteOptionsV1>() - 1) as u32;
        metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the readable allocation is complete, but its declared prefix is short.
        assert_eq!(
            unsafe {
                write(
                    operator,
                    &path,
                    &payload,
                    &write_options,
                    &mut metadata,
                    &mut error,
                )
            },
            STATUS_ABI_MISMATCH,
        );
        assert!(metadata.is_null());
        assert!(error.is_null());

        let invalid_utf8_bytes = [0xFF];
        let invalid_utf8 = bytes(&invalid_utf8_bytes);
        buffer = NonNull::<BufferV1>::dangling().as_ptr();
        error = ptr::null_mut();
        // SAFETY: the byte view is valid but intentionally not UTF-8.
        assert_eq!(
            unsafe {
                read(
                    operator,
                    &invalid_utf8,
                    ptr::null(),
                    MAX_OUTPUT_BYTES,
                    &mut buffer,
                    &mut error,
                )
            },
            STATUS_ERROR,
        );
        assert!(buffer.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_INVALID_ARGUMENT);

        // SAFETY: both handles remain uniquely owned by this test.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn blocking_core_memory_operations_cover_success_missing_and_recursive_delete() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let check = api.operator_check.expect("WHOLE_OBJECT check is installed");
        let create_dir = api
            .operator_create_dir
            .expect("WHOLE_OBJECT create_dir is installed");
        let delete = api
            .operator_delete
            .expect("WHOLE_OBJECT delete is installed");
        let stat = api.operator_stat.expect("WHOLE_OBJECT stat is installed");
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();

        // SAFETY: the operator is live and the optional error slot is writable.
        assert_eq!(unsafe { check(operator, &mut error) }, STATUS_OK);
        assert!(error.is_null());

        assert!(!memory_object_exists(&api, operator, b"phase2/file"));
        write_memory_object(&api, operator, b"phase2/file", b"phase-two");
        assert!(memory_object_exists(&api, operator, b"phase2/file"));
        assert_eq!(
            stat_mode_and_len(&api, operator, b"phase2/file"),
            (ENTRY_MODE_FILE, 9),
        );

        let directory = bytes(b"phase2/tree/");
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the operator, path carrier, and output slot are valid.
        assert_eq!(
            unsafe { create_dir(operator, &directory, &mut error) },
            STATUS_OK,
        );
        assert!(error.is_null());
        assert_eq!(
            stat_mode_and_len(&api, operator, b"phase2/tree/"),
            (ENTRY_MODE_DIRECTORY, 0),
        );

        let file = bytes(b"phase2/file");
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: default delete borrows valid carrier storage for this call only.
        assert_eq!(
            unsafe { delete(operator, &file, ptr::null(), &mut error) },
            STATUS_OK,
        );
        assert!(error.is_null());
        assert!(!memory_object_exists(&api, operator, b"phase2/file"));

        let nested_directory = bytes(b"phase2/tree/nested/");
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the nested directory carrier and output slot are valid.
        assert_eq!(
            unsafe { create_dir(operator, &nested_directory, &mut error) },
            STATUS_OK,
        );
        assert!(error.is_null());
        write_memory_object(&api, operator, b"phase2/tree/nested/child", b"recursive");
        assert!(memory_object_exists(
            &api,
            operator,
            b"phase2/tree/nested/child",
        ));

        let absent = bytes(b"");
        let recursive = DeleteOptionsV1 {
            struct_size: size_of::<DeleteOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 0,
            flags: DELETE_RECURSIVE,
            version: absent,
        };
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: options and path are complete readable v1 carriers.
        assert_eq!(
            unsafe { delete(operator, &directory, &recursive, &mut error) },
            STATUS_OK,
        );
        assert!(error.is_null());
        assert!(!memory_object_exists(
            &api,
            operator,
            b"phase2/tree/nested/child",
        ));
        assert!(!memory_object_exists(&api, operator, b"phase2/tree/"));

        let missing = bytes(b"phase2/missing");
        let mut metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: required outputs are writable and the missing path is valid UTF-8.
        assert_eq!(
            unsafe { stat(operator, &missing, ptr::null(), &mut metadata, &mut error) },
            STATUS_ERROR,
        );
        assert!(metadata.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_NOT_FOUND);

        // SAFETY: both handles remain uniquely owned by this test.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn blocking_core_options_reject_unknown_and_noncanonical_fields_atomically() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let stat = api.operator_stat.expect("WHOLE_OBJECT stat is installed");
        let delete = api
            .operator_delete
            .expect("WHOLE_OBJECT delete is installed");
        let path = bytes(b"phase2/options-target");
        let absent = bytes(b"");
        write_memory_object(&api, operator, b"phase2/options-target", b"kept");

        let mut stat_options = StatOptionsV1 {
            struct_size: size_of::<StatOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 1 << 63,
            version: absent,
            if_match: absent,
            if_none_match: absent,
        };
        let mut metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the complete options carrier deliberately contains an unknown bit.
        assert_eq!(
            unsafe { stat(operator, &path, &stat_options, &mut metadata, &mut error) },
            STATUS_ABI_MISMATCH,
        );
        assert!(metadata.is_null());
        assert!(error.is_null());

        stat_options.present_bits = 0;
        stat_options.version = bytes(b"noncanonical-absent");
        metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: version is readable but intentionally noncanonical while absent.
        assert_eq!(
            unsafe { stat(operator, &path, &stat_options, &mut metadata, &mut error) },
            STATUS_ABI_MISMATCH,
        );
        assert!(metadata.is_null());
        assert!(error.is_null());

        let mut delete_options = DeleteOptionsV1 {
            struct_size: size_of::<DeleteOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 1 << 63,
            flags: 0,
            version: absent,
        };
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: options are complete and the unknown presence bit is intentional.
        assert_eq!(
            unsafe { delete(operator, &path, &delete_options, &mut error) },
            STATUS_ABI_MISMATCH,
        );
        assert!(error.is_null());
        assert!(memory_object_exists(
            &api,
            operator,
            b"phase2/options-target",
        ));

        delete_options.present_bits = 0;
        delete_options.flags = 1 << 63;
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: options are complete and the unknown flag is intentional.
        assert_eq!(
            unsafe { delete(operator, &path, &delete_options, &mut error) },
            STATUS_ABI_MISMATCH,
        );
        assert!(error.is_null());
        assert!(memory_object_exists(
            &api,
            operator,
            b"phase2/options-target",
        ));

        delete_options.flags = 0;
        delete_options.version = bytes(b"noncanonical-absent");
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the absent version carrier is deliberately noncanonical.
        assert_eq!(
            unsafe { delete(operator, &path, &delete_options, &mut error) },
            STATUS_ABI_MISMATCH,
        );
        assert!(error.is_null());
        assert!(memory_object_exists(
            &api,
            operator,
            b"phase2/options-target",
        ));

        // SAFETY: both handles remain uniquely owned by this test.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn listing_streams_snapshots_preserves_state_and_outlives_operator() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        write_memory_object(&api, operator, b"listing/a.txt", b"a");
        write_memory_object(&api, operator, b"listing/nested/b.bin", b"bb");
        write_memory_object(&api, operator, b"listing/nested/deeper/c.txt", b"ccc");

        let default_lister = memory_lister(&api, operator, b"listing/", ptr::null());
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: a NULL required entry output is rejected before advancing the lister.
        assert_eq!(
            unsafe {
                api.lister_next.expect("LISTING next is installed")(
                    default_lister,
                    ptr::null_mut(),
                    &mut error,
                )
            },
            STATUS_ABI_MISMATCH,
        );
        assert!(error.is_null());

        let default_entries = collect_lister(&api, default_lister);
        assert!(default_entries.iter().any(|entry| {
            entry.path == "listing/a.txt"
                && entry.name == "a.txt"
                && entry.mode == ENTRY_MODE_FILE
                && entry.content_length == 1
        }));
        assert!(default_entries.iter().any(|entry| {
            entry.path == "listing/nested/"
                && entry.name == "nested/"
                && entry.mode == ENTRY_MODE_DIRECTORY
        }));
        assert!(
            default_entries
                .iter()
                .all(|entry| entry.path != "listing/nested/b.bin")
        );
        // SAFETY: close is idempotent and free consumes the outer handle once.
        unsafe {
            api.lister_close.expect("LISTING close is installed")(default_lister);
            api.lister_close.expect("LISTING close is installed")(default_lister);
            api.lister_free.expect("LISTING free is installed")(default_lister);
        }

        let absent = bytes(b"");
        let recursive = ListOptionsV1 {
            struct_size: size_of::<ListOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 0,
            flags: LIST_RECURSIVE,
            limit: 0,
            start_after: absent,
        };
        let recursive_lister = memory_lister(&api, operator, b"listing/", &recursive);

        // The OpenDAL blocking lister owns everything it needs after creation.
        // SAFETY: the two constructor outputs are still uniquely owned here.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
        let recursive_entries = collect_lister(&api, recursive_lister);
        assert!(recursive_entries.iter().any(|entry| {
            entry.path == "listing/nested/b.bin"
                && entry.name == "b.bin"
                && entry.mode == ENTRY_MODE_FILE
                && entry.content_length == 2
        }));
        assert!(
            recursive_entries
                .iter()
                .any(|entry| entry.path == "listing/nested/deeper/c.txt")
        );

        let next = api.lister_next.expect("LISTING next is installed");
        let mut entry = NonNull::<EntryV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: exhausted listers deterministically keep returning END.
        assert_eq!(
            unsafe { next(recursive_lister, &mut entry, &mut error) },
            STATUS_END,
        );
        assert!(entry.is_null());
        assert!(error.is_null());

        // SAFETY: closing retains the outer handle for deterministic state checks.
        unsafe {
            api.lister_close.expect("LISTING close is installed")(recursive_lister);
            api.lister_close.expect("LISTING close is installed")(recursive_lister);
        }
        entry = NonNull::<EntryV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: outputs are writable and the closed handle remains live.
        assert_eq!(
            unsafe { next(recursive_lister, &mut entry, &mut error) },
            STATUS_ERROR,
        );
        assert!(entry.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        // SAFETY: NULL destruction is a no-op; the live handle is freed once.
        unsafe {
            api.lister_close.expect("LISTING close is installed")(ptr::null_mut());
            api.lister_free.expect("LISTING free is installed")(ptr::null_mut());
            api.entry_free.expect("BASE entry free is installed")(ptr::null_mut());
            api.lister_free.expect("LISTING free is installed")(recursive_lister);
        }
    }

    #[test]
    fn listing_options_reject_noncanonical_inputs_atomically() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let constructor = api
            .operator_lister
            .expect("LISTING constructor is installed");
        let path = bytes(b"listing-options/");
        let absent = bytes(b"");

        let mut required_output_error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the required lister output is deliberately NULL; the valid
        // error slot must nevertheless be independently cleared.
        assert_eq!(
            unsafe {
                constructor(
                    operator,
                    &path,
                    ptr::null(),
                    ptr::null_mut(),
                    &mut required_output_error,
                )
            },
            STATUS_ABI_MISMATCH,
        );
        assert!(required_output_error.is_null());

        let assert_abi_mismatch = |options: &ListOptionsV1| {
            let mut lister = NonNull::<ListerV1>::dangling().as_ptr();
            let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
            // SAFETY: the complete carrier intentionally violates one ABI invariant.
            assert_eq!(
                unsafe { constructor(operator, &path, options, &mut lister, &mut error) },
                STATUS_ABI_MISMATCH,
            );
            assert!(lister.is_null());
            assert!(error.is_null());
        };

        let mut options = ListOptionsV1 {
            struct_size: size_of::<ListOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 1 << 63,
            flags: 0,
            limit: 0,
            start_after: absent,
        };
        assert_abi_mismatch(&options);

        options.present_bits = 0;
        options.flags = 1 << 63;
        assert_abi_mismatch(&options);

        options.flags = 0;
        options.limit = 1;
        assert_abi_mismatch(&options);

        options.limit = 0;
        options.start_after = bytes(b"noncanonical-absent");
        assert_abi_mismatch(&options);

        options.present_bits = LIST_LIMIT_PRESENT;
        options.limit = u64::MAX;
        options.start_after = absent;
        assert_abi_mismatch(&options);

        options.present_bits = 0;
        options.limit = 0;
        options.struct_version = STRUCT_VERSION + 1;
        assert_abi_mismatch(&options);

        let invalid_utf8_bytes = [0xFF];
        options.struct_version = STRUCT_VERSION;
        options.present_bits = LIST_START_AFTER_PRESENT;
        options.start_after = bytes(&invalid_utf8_bytes);
        let mut lister = NonNull::<ListerV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the start_after bytes are readable but intentionally invalid UTF-8.
        assert_eq!(
            unsafe { constructor(operator, &path, &options, &mut lister, &mut error) },
            STATUS_ERROR,
        );
        assert!(lister.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_INVALID_ARGUMENT);

        options.present_bits = LIST_LIMIT_PRESENT | LIST_START_AFTER_PRESENT;
        options.flags = LIST_RECURSIVE;
        options.limit = 0;
        options.start_after = absent;
        let valid_lister = memory_lister(&api, operator, b"listing-options/", &options);
        // SAFETY: the valid empty listing can be released before iteration.
        unsafe {
            api.lister_free.expect("LISTING free is installed")(valid_lister);
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn listing_error_panic_and_poison_are_terminal_and_reported_once() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        write_memory_object(&api, operator, b"listing-terminal/item", b"value");
        let next = api.lister_next.expect("LISTING next is installed");
        let free = api.lister_free.expect("LISTING free is installed");

        let error_lister = memory_lister(&api, operator, b"listing-terminal/", ptr::null());
        install_lister_next_test_mode(error_lister, TEST_LISTER_NEXT_ERROR);
        let mut entry = NonNull::<EntryV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the test-only hook injects an ordinary fallible next result.
        assert_eq!(
            unsafe { next(error_lister, &mut entry, &mut error) },
            STATUS_ERROR,
        );
        assert!(entry.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_UNEXPECTED);
        entry = NonNull::<EntryV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: an ordinary next error exhausts the handle after one report.
        assert_eq!(
            unsafe { next(error_lister, &mut entry, &mut error) },
            STATUS_END,
        );
        assert!(entry.is_null());
        assert!(error.is_null());

        let panic_lister = memory_lister(&api, operator, b"listing-terminal/", ptr::null());
        install_lister_next_test_mode(panic_lister, TEST_LISTER_NEXT_PANIC);
        entry = NonNull::<EntryV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the injected panic is contained inside the ABI boundary.
        assert_eq!(
            unsafe { next(panic_lister, &mut entry, &mut error) },
            STATUS_PANIC,
        );
        assert!(entry.is_null());
        assert!(error.is_null());
        entry = NonNull::<EntryV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: panic also leaves the handle deterministically exhausted.
        assert_eq!(
            unsafe { next(panic_lister, &mut entry, &mut error) },
            STATUS_END,
        );
        assert!(entry.is_null());
        assert!(error.is_null());

        let poisoned_lister = memory_lister(&api, operator, b"listing-terminal/", ptr::null());
        let poisoned = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: this test owns the live handle and intentionally poisons
            // its state mutex while it still contains an Open lister.
            let lister = unsafe { &*poisoned_lister };
            let _guard = lister.state.lock().expect("fresh lister lock is healthy");
            panic!("intentionally poison lister state");
        }));
        assert!(poisoned.is_err());
        entry = NonNull::<EntryV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: poisoned Open state is consumed without resuming iteration.
        assert_eq!(
            unsafe { next(poisoned_lister, &mut entry, &mut error) },
            STATUS_ERROR,
        );
        assert!(entry.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_UNEXPECTED);
        entry = NonNull::<EntryV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: poison is cleared only after replacing Open with Exhausted.
        assert_eq!(
            unsafe { next(poisoned_lister, &mut entry, &mut error) },
            STATUS_END,
        );
        assert!(entry.is_null());
        assert!(error.is_null());

        // SAFETY: all three listers and constructor outputs are uniquely owned.
        unsafe {
            free(error_lister);
            free(panic_lister);
            free(poisoned_lister);
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn random_reader_ranges_are_independent_concurrent_and_outlive_operator() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        write_memory_object(&api, operator, b"reader/data.bin", b"0123456789");
        let reader = memory_reader(&api, operator, b"reader/data.bin", ptr::null());

        // OpenDAL readers own the state needed after construction.
        // SAFETY: both constructor outputs remain uniquely owned here.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }

        assert_eq!(
            read_reader_bytes(&api, reader, &byte_range(RANGE_FULL, 0, 0), 64),
            b"0123456789",
        );
        assert_eq!(
            read_reader_bytes(&api, reader, &byte_range(RANGE_FROM, 3, 0), 64),
            b"3456789",
        );
        assert_eq!(
            read_reader_bytes(&api, reader, &byte_range(RANGE_OFFSET_LENGTH, 2, 4), 64,),
            b"2345",
        );
        assert_eq!(
            read_reader_bytes(&api, reader, &byte_range(RANGE_SUFFIX, 0, 3), 64),
            b"789",
        );

        // Shared reader calls take read locks, so independent ranges can execute together.
        let reader_ref = unsafe { &*reader };
        let (left, right) = std::thread::scope(|scope| {
            let left = scope.spawn(|| {
                read_reader_bytes(
                    &api,
                    ptr::from_ref(reader_ref).cast_mut(),
                    &byte_range(RANGE_OFFSET_LENGTH, 0, 5),
                    64,
                )
            });
            let right = scope.spawn(|| {
                read_reader_bytes(
                    &api,
                    ptr::from_ref(reader_ref).cast_mut(),
                    &byte_range(RANGE_OFFSET_LENGTH, 5, 5),
                    64,
                )
            });
            (
                left.join().expect("left range read did not panic"),
                right.join().expect("right range read did not panic"),
            )
        });
        assert_eq!(left, b"01234");
        assert_eq!(right, b"56789");

        let read = api.reader_read.expect("RANDOM_READER read is installed");
        let oversized = byte_range(RANGE_OFFSET_LENGTH, 0, 6);
        let mut buffer = NonNull::<BufferV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the explicit range exceeds the negotiated cap before native allocation.
        assert_eq!(
            unsafe { read(reader, &oversized, 5, &mut buffer, &mut error) },
            STATUS_ERROR,
        );
        assert!(buffer.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_BUFFER_TOO_LARGE);

        let overflow = byte_range(RANGE_OFFSET_LENGTH, u64::MAX, 1);
        buffer = NonNull::<BufferV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: overflow is an ABI-only malformed range and leaves outputs clear.
        assert_eq!(
            unsafe { read(reader, &overflow, 64, &mut buffer, &mut error) },
            STATUS_ABI_MISMATCH,
        );
        assert!(buffer.is_null());
        assert!(error.is_null());

        // Neither a size rejection nor malformed input closes a valid reader.
        assert_eq!(
            read_reader_bytes(&api, reader, &byte_range(RANGE_OFFSET_LENGTH, 1, 2), 64,),
            b"12",
        );

        // SAFETY: close is idempotent and keeps the outer handle alive.
        unsafe {
            api.reader_close.expect("RANDOM_READER close is installed")(reader);
            api.reader_close.expect("RANDOM_READER close is installed")(reader);
        }
        buffer = NonNull::<BufferV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        let full = byte_range(RANGE_FULL, 0, 0);
        // SAFETY: the closed handle remains live for deterministic state reporting.
        assert_eq!(
            unsafe { read(reader, &full, 64, &mut buffer, &mut error) },
            STATUS_ERROR,
        );
        assert!(buffer.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        // SAFETY: NULL destruction is a no-op; the live handle is freed once.
        unsafe {
            api.reader_close.expect("RANDOM_READER close is installed")(ptr::null_mut());
            api.reader_free.expect("RANDOM_READER free is installed")(ptr::null_mut());
            api.reader_free.expect("RANDOM_READER free is installed")(reader);
        }
    }

    #[test]
    fn random_reader_options_error_panic_and_poison_preserve_terminal_rules() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        write_memory_object(&api, operator, b"reader/state.bin", b"state");
        let constructor = api
            .operator_reader
            .expect("RANDOM_READER constructor is installed");
        let path = bytes(b"reader/state.bin");
        let absent = bytes(b"");

        let assert_abi_mismatch = |options: &ReaderOptionsV1| {
            let mut reader = NonNull::<ReaderV1>::dangling().as_ptr();
            let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
            // SAFETY: the complete options carrier deliberately violates one ABI invariant.
            assert_eq!(
                unsafe { constructor(operator, &path, options, &mut reader, &mut error) },
                STATUS_ABI_MISMATCH,
            );
            assert!(reader.is_null());
            assert!(error.is_null());
        };

        let mut options = ReaderOptionsV1 {
            struct_size: size_of::<ReaderOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 1 << 63,
            version: absent,
            if_match: absent,
            if_none_match: absent,
        };
        assert_abi_mismatch(&options);
        options.present_bits = 0;
        options.version = bytes(b"noncanonical-absent");
        assert_abi_mismatch(&options);
        options.version = absent;
        options.struct_version = STRUCT_VERSION + 1;
        assert_abi_mismatch(&options);

        let invalid_utf8 = [0xFF];
        options.struct_version = STRUCT_VERSION;
        options.present_bits = READER_IF_MATCH_PRESENT;
        options.if_match = bytes(&invalid_utf8);
        let mut invalid_reader = NonNull::<ReaderV1>::dangling().as_ptr();
        let mut invalid_error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the present text view is readable but intentionally invalid UTF-8.
        assert_eq!(
            unsafe {
                constructor(
                    operator,
                    &path,
                    &options,
                    &mut invalid_reader,
                    &mut invalid_error,
                )
            },
            STATUS_ERROR,
        );
        assert!(invalid_reader.is_null());
        assert_eq!(take_error_kind(&api, invalid_error), ERROR_INVALID_ARGUMENT);

        let reader = memory_reader(&api, operator, b"reader/state.bin", ptr::null());
        let read = api.reader_read.expect("RANDOM_READER read is installed");
        let full = byte_range(RANGE_FULL, 0, 0);
        install_reader_read_test_mode(reader, TEST_READER_READ_ERROR);
        let mut buffer = NonNull::<BufferV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the test hook injects an ordinary operation failure.
        assert_eq!(
            unsafe { read(reader, &full, 64, &mut buffer, &mut error) },
            STATUS_ERROR,
        );
        assert!(buffer.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_UNEXPECTED);
        assert_eq!(read_reader_bytes(&api, reader, &full, 64), b"state");

        install_reader_read_test_mode(reader, TEST_READER_READ_PANIC);
        buffer = NonNull::<BufferV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the injected panic is contained and closes uncertain state.
        assert_eq!(
            unsafe { read(reader, &full, 64, &mut buffer, &mut error) },
            STATUS_PANIC,
        );
        assert!(buffer.is_null());
        assert!(error.is_null());
        buffer = NonNull::<BufferV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: a contained panic leaves the handle deterministically closed.
        assert_eq!(
            unsafe { read(reader, &full, 64, &mut buffer, &mut error) },
            STATUS_ERROR,
        );
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        let poisoned_reader = memory_reader(&api, operator, b"reader/state.bin", ptr::null());
        let poisoned = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: this test owns the live handle and intentionally poisons its state lock.
            let reader = unsafe { &*poisoned_reader };
            let _guard = reader.state.write().expect("fresh reader lock is healthy");
            panic!("intentionally poison reader state");
        }));
        assert!(poisoned.is_err());
        buffer = NonNull::<BufferV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: poison is reported once while transitioning to Closed.
        assert_eq!(
            unsafe { read(poisoned_reader, &full, 64, &mut buffer, &mut error) },
            STATUS_ERROR,
        );
        assert_eq!(take_error_kind(&api, error), ERROR_UNEXPECTED);
        buffer = NonNull::<BufferV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: later calls observe the deterministic terminal state.
        assert_eq!(
            unsafe { read(poisoned_reader, &full, 64, &mut buffer, &mut error) },
            STATUS_ERROR,
        );
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        // SAFETY: all resources remain uniquely owned by this test.
        unsafe {
            api.reader_free.expect("RANDOM_READER free is installed")(reader);
            api.reader_free.expect("RANDOM_READER free is installed")(poisoned_reader);
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn read_stream_is_bounded_range_aware_and_has_stable_end() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let path = "streams/数据.bin".as_bytes();
        write_memory_object(&api, operator, path, b"0123456789");
        write_memory_object(&api, operator, b"streams/empty.bin", b"");

        let cases = [
            (byte_range(RANGE_FULL, 0, 0), b"0123456789".as_slice()),
            (byte_range(RANGE_FROM, 3, 0), b"3456789".as_slice()),
            (byte_range(RANGE_OFFSET_LENGTH, 2, 5), b"23456".as_slice()),
            (byte_range(RANGE_SUFFIX, 0, 3), b"789".as_slice()),
        ];
        for (range, expected) in cases {
            let options = read_stream_options(range, 4);
            let stream = memory_read_stream(&api, operator, path, &options);
            let chunks = collect_read_stream(&api, stream);
            assert!(
                chunks
                    .iter()
                    .all(|chunk| !chunk.is_empty() && chunk.len() <= 4)
            );
            assert_eq!(chunks.concat(), expected);

            let mut buffer = NonNull::<BufferV1>::dangling().as_ptr();
            let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
            // SAFETY: EOF is stable while the stream remains live and unclosed.
            assert_eq!(
                unsafe {
                    api.read_stream_next.expect("READ_STREAM next is installed")(
                        stream,
                        MAX_OUTPUT_BYTES,
                        &mut buffer,
                        &mut error,
                    )
                },
                STATUS_END,
            );
            assert!(buffer.is_null());
            assert!(error.is_null());

            // SAFETY: close and free are idempotent/unique as documented.
            unsafe {
                api.read_stream_close
                    .expect("READ_STREAM close is installed")(stream);
                api.read_stream_close
                    .expect("READ_STREAM close is installed")(stream);
            }
            buffer = NonNull::<BufferV1>::dangling().as_ptr();
            error = NonNull::<ErrorV1>::dangling().as_ptr();
            // SAFETY: closed handles remain live for deterministic state reporting.
            assert_eq!(
                unsafe {
                    api.read_stream_next.expect("READ_STREAM next is installed")(
                        stream,
                        MAX_OUTPUT_BYTES,
                        &mut buffer,
                        &mut error,
                    )
                },
                STATUS_ERROR,
            );
            assert!(buffer.is_null());
            assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);
            unsafe {
                api.read_stream_free.expect("READ_STREAM free is installed")(stream);
            }
        }

        let empty = memory_read_stream(
            &api,
            operator,
            b"streams/empty.bin",
            &read_stream_options(byte_range(RANGE_FULL, 0, 0), 4),
        );
        assert!(collect_read_stream(&api, empty).is_empty());

        let outliving = memory_read_stream(
            &api,
            operator,
            path,
            &read_stream_options(byte_range(RANGE_FULL, 0, 0), 3),
        );
        // SAFETY: streams own all state required after their originating Operator is freed.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
        let chunks = collect_read_stream(&api, outliving);
        assert_eq!(
            chunks.iter().map(Vec::len).collect::<Vec<_>>(),
            [3, 3, 3, 1]
        );
        assert_eq!(chunks.concat(), b"0123456789");

        // SAFETY: each stream handle is freed exactly once; NULL is a no-op.
        unsafe {
            api.read_stream_close
                .expect("READ_STREAM close is installed")(ptr::null_mut());
            api.read_stream_free.expect("READ_STREAM free is installed")(ptr::null_mut());
            api.read_stream_free.expect("READ_STREAM free is installed")(empty);
            api.read_stream_free.expect("READ_STREAM free is installed")(outliving);
        }
    }

    #[test]
    fn read_stream_rejects_invalid_bounds_and_fails_terminally() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        write_memory_object(&api, operator, b"streams/bounds.bin", b"bounded");
        let constructor = api
            .operator_read_stream
            .expect("READ_STREAM constructor is installed");
        let path = bytes(b"streams/bounds.bin");

        for chunk_size in [0, MAX_OUTPUT_BYTES + 1] {
            let options = read_stream_options(byte_range(RANGE_FULL, 0, 0), chunk_size);
            let mut stream = NonNull::<ReadStreamV1>::dangling().as_ptr();
            let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
            // SAFETY: complete options carry one deliberately invalid semantic value.
            assert_eq!(
                unsafe { constructor(operator, &path, &options, &mut stream, &mut error) },
                STATUS_ERROR,
            );
            assert!(stream.is_null());
            assert_eq!(take_error_kind(&api, error), ERROR_INVALID_ARGUMENT);
        }

        let mut malformed = read_stream_options(byte_range(RANGE_FULL, 0, 0), 4);
        malformed.present_bits = 1 << 63;
        let mut stream = NonNull::<ReadStreamV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the complete options deliberately contain an unknown ABI bit.
        assert_eq!(
            unsafe { constructor(operator, &path, &malformed, &mut stream, &mut error) },
            STATUS_ABI_MISMATCH,
        );
        assert!(stream.is_null());
        assert!(error.is_null());

        let stream = memory_read_stream(
            &api,
            operator,
            b"streams/bounds.bin",
            &read_stream_options(byte_range(RANGE_FULL, 0, 0), 4),
        );
        let next = api.read_stream_next.expect("READ_STREAM next is installed");
        let mut buffer = NonNull::<BufferV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the per-call ceiling is intentionally below the fixed chunk bound.
        assert_eq!(
            unsafe { next(stream, 2, &mut buffer, &mut error) },
            STATUS_ERROR,
        );
        assert!(buffer.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_BUFFER_TOO_LARGE);

        buffer = NonNull::<BufferV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: a bound failure is terminal because the cursor already advanced.
        assert_eq!(
            unsafe { next(stream, MAX_OUTPUT_BYTES, &mut buffer, &mut error) },
            STATUS_ERROR,
        );
        assert!(buffer.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        // SAFETY: all resources are uniquely owned here.
        unsafe {
            api.read_stream_free.expect("READ_STREAM free is installed")(stream);
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn copy_and_rename_preserve_backend_semantics_and_atomic_outputs() {
        let api = api();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time is after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "opendal-mbt-copy-rename-{}-{unique}",
            std::process::id(),
        ));
        let (operator, info) = filesystem_operator(&api, &root);
        write_memory_object(&api, operator, b"moves/source.bin", b"copy-rename");
        let copy = api.operator_copy.expect("WHOLE_OBJECT copy is installed");
        let rename = api
            .operator_rename
            .expect("WHOLE_OBJECT rename is installed");
        let source = bytes(b"moves/source.bin");
        let copied = bytes(b"moves/copied.bin");
        let renamed = bytes(b"moves/renamed.bin");

        let mut metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: all path carriers and output slots remain live for this call.
        assert_eq!(
            unsafe { copy(operator, &source, &copied, &mut metadata, &mut error) },
            STATUS_OK,
        );
        assert!(error.is_null());
        let _ = take_metadata_content_length(&api, metadata);
        assert_eq!(
            read_object(&api, operator, b"moves/source.bin"),
            b"copy-rename"
        );
        assert_eq!(
            read_object(&api, operator, b"moves/copied.bin"),
            b"copy-rename"
        );

        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: source/destination and optional error output are valid.
        assert_eq!(
            unsafe { rename(operator, &copied, &renamed, &mut error) },
            STATUS_OK,
        );
        assert!(error.is_null());
        assert!(!memory_object_exists(&api, operator, b"moves/copied.bin"));
        assert_eq!(
            read_object(&api, operator, b"moves/renamed.bin"),
            b"copy-rename"
        );

        let missing = bytes(b"moves/missing.bin");
        metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: a backend NotFound must clear metadata and return one owned error.
        assert_eq!(
            unsafe { copy(operator, &missing, &copied, &mut metadata, &mut error) },
            STATUS_ERROR,
        );
        assert!(metadata.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_NOT_FOUND);

        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the required metadata output is deliberately NULL.
        assert_eq!(
            unsafe { copy(operator, &source, &copied, ptr::null_mut(), &mut error) },
            STATUS_ABI_MISMATCH,
        );
        assert!(error.is_null());

        let invalid_utf8 = [0xFF];
        let invalid_destination = bytes(&invalid_utf8);
        metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the destination bytes are readable but intentionally invalid UTF-8.
        assert_eq!(
            unsafe {
                copy(
                    operator,
                    &source,
                    &invalid_destination,
                    &mut metadata,
                    &mut error,
                )
            },
            STATUS_ERROR,
        );
        assert!(metadata.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_INVALID_ARGUMENT);

        // SAFETY: both constructor outputs remain uniquely owned.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
        std::fs::remove_dir_all(&root).expect("isolated filesystem root is removable");
    }

    #[test]
    fn chunked_writer_commits_chunks_outlives_operator_and_is_terminal() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let writer = memory_writer(&api, operator, b"writer/outlives.bin", ptr::null());

        // The Writer owns all state needed after construction.
        // SAFETY: both constructor outputs remain uniquely owned here.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }

        write_writer_chunk(&api, writer, b"chunked-");
        write_writer_chunk(&api, writer, b"writer");
        let metadata = finish_writer(&api, writer);
        assert_eq!(take_metadata_content_length(&api, metadata), 14);

        let close = api.writer_close.expect("CHUNKED_WRITER close is installed");
        let write = api.writer_write.expect("CHUNKED_WRITER write is installed");
        let mut repeated_metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the closed outer handle and output slots remain live.
        assert_eq!(
            unsafe { close(writer, &mut repeated_metadata, &mut error) },
            STATUS_ERROR,
        );
        assert!(repeated_metadata.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        let data = bytes(b"late");
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the closed outer handle remains live for deterministic reporting.
        assert_eq!(unsafe { write(writer, &data, &mut error) }, STATUS_ERROR);
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        // SAFETY: NULL destruction is a no-op and the live handle is freed once.
        unsafe {
            api.writer_free.expect("CHUNKED_WRITER free is installed")(ptr::null_mut());
            api.writer_free.expect("CHUNKED_WRITER free is installed")(writer);
        }
    }

    #[test]
    fn writer_abort_is_explicit_idempotent_and_terminal() {
        use std::sync::atomic::Ordering;

        let api = api();
        let (operator, info) = memory_operator(&api);
        let writer = memory_writer(&api, operator, b"writer/aborted.bin", ptr::null());
        write_writer_chunk(&api, writer, b"discard-me");
        let aborts_before = TEST_WRITER_NATIVE_ABORTS.load(Ordering::Relaxed);
        abort_writer(&api, writer);
        abort_writer(&api, writer);
        assert_eq!(
            TEST_WRITER_NATIVE_ABORTS.load(Ordering::Relaxed),
            aborts_before + 1,
        );
        assert!(!memory_object_exists(&api, operator, b"writer/aborted.bin"));

        let write = api.writer_write.expect("CHUNKED_WRITER write is installed");
        let close = api.writer_close.expect("CHUNKED_WRITER close is installed");
        let abort = api
            .writer_abort
            .expect("WRITER_ABORT operation is installed");
        let data = bytes(b"late");
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: an aborted live handle deterministically rejects writes.
        assert_eq!(unsafe { write(writer, &data, &mut error) }, STATUS_ERROR);
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);
        let mut metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: an aborted live handle deterministically rejects finish.
        assert_eq!(
            unsafe { close(writer, &mut metadata, &mut error) },
            STATUS_ERROR,
        );
        assert!(metadata.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        let finished = memory_writer(&api, operator, b"writer/finished.bin", ptr::null());
        write_writer_chunk(&api, finished, b"committed");
        let metadata = finish_writer(&api, finished);
        assert_eq!(take_metadata_content_length(&api, metadata), 9);
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: abort after a successful finish is not reported as cleanup.
        assert_eq!(unsafe { abort(finished, &mut error) }, STATUS_ERROR);
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        // SAFETY: all resources remain uniquely owned.
        unsafe {
            api.writer_free.expect("CHUNKED_WRITER free is installed")(writer);
            api.writer_free.expect("CHUNKED_WRITER free is installed")(finished);
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn writer_abort_failure_and_panic_are_terminal() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let abort = api
            .writer_abort
            .expect("WRITER_ABORT operation is installed");
        let write = api.writer_write.expect("CHUNKED_WRITER write is installed");
        let free = api.writer_free.expect("CHUNKED_WRITER free is installed");
        let data = bytes(b"late");

        let abort_error = memory_writer(&api, operator, b"writer/abort-error", ptr::null());
        install_writer_call_test_mode(abort_error, TEST_WRITER_ABORT_ERROR);
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the test hook injects one ordinary native abort failure.
        assert_eq!(unsafe { abort(abort_error, &mut error) }, STATUS_ERROR);
        assert_eq!(take_error_kind(&api, error), ERROR_UNEXPECTED);
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: a failed abort attempt is terminal rather than idempotent.
        assert_eq!(unsafe { abort(abort_error, &mut error) }, STATUS_ERROR);
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        let abort_panic = memory_writer(&api, operator, b"writer/abort-panic", ptr::null());
        install_writer_call_test_mode(abort_panic, TEST_WRITER_ABORT_PANIC);
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the injected panic is contained and leaves a terminal handle.
        assert_eq!(unsafe { abort(abort_panic, &mut error) }, STATUS_PANIC);
        assert!(error.is_null());
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the terminal handle rejects later writes.
        assert_eq!(
            unsafe { write(abort_panic, &data, &mut error) },
            STATUS_ERROR
        );
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        // SAFETY: all resources remain uniquely owned.
        unsafe {
            free(abort_error);
            free(abort_panic);
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn chunked_writer_serializes_same_handle_calls_and_free_never_closes() {
        use std::sync::atomic::Ordering;

        let api = api();
        let (operator, info) = memory_operator(&api);
        let writer = memory_writer(&api, operator, b"writer/concurrent.bin", ptr::null());
        let writer_ref = unsafe { &*writer };
        std::thread::scope(|scope| {
            let left = scope.spawn(|| {
                write_writer_chunk(&api, ptr::from_ref(writer_ref).cast_mut(), b"left");
            });
            let right = scope.spawn(|| {
                write_writer_chunk(&api, ptr::from_ref(writer_ref).cast_mut(), b"right");
            });
            left.join().expect("left writer call did not panic");
            right.join().expect("right writer call did not panic");
        });
        let metadata = finish_writer(&api, writer);
        assert_eq!(take_metadata_content_length(&api, metadata), 9);
        let content = read_object(&api, operator, b"writer/concurrent.bin");
        assert!(content == b"leftright" || content == b"rightleft");

        let unfinished = memory_writer(&api, operator, b"writer/unfinished.bin", ptr::null());
        write_writer_chunk(&api, unfinished, b"not-finished");
        let closes_before = TEST_WRITER_NATIVE_CLOSES.load(Ordering::Relaxed);
        // SAFETY: freeing this uniquely owned open writer must only drop it.
        unsafe { api.writer_free.expect("CHUNKED_WRITER free is installed")(unfinished) };
        assert_eq!(
            TEST_WRITER_NATIVE_CLOSES.load(Ordering::Relaxed),
            closes_before,
        );

        // SAFETY: all remaining resources are uniquely owned.
        unsafe {
            api.writer_free.expect("CHUNKED_WRITER free is installed")(writer);
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn chunked_writer_options_and_inputs_fail_atomically() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let constructor = api
            .operator_writer
            .expect("CHUNKED_WRITER constructor is installed");
        let path = bytes(b"writer/options.bin");
        let absent = bytes(b"");

        let mut required_output_error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the required writer output is NULL while the error output is valid.
        assert_eq!(
            unsafe {
                constructor(
                    operator,
                    &path,
                    ptr::null(),
                    ptr::null_mut(),
                    &mut required_output_error,
                )
            },
            STATUS_ABI_MISMATCH,
        );
        assert!(required_output_error.is_null());

        let assert_abi_mismatch = |options: &WriteOptionsV1| {
            let mut writer = NonNull::<WriterV1>::dangling().as_ptr();
            let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
            // SAFETY: the complete options carrier intentionally violates one invariant.
            assert_eq!(
                unsafe { constructor(operator, &path, options, &mut writer, &mut error) },
                STATUS_ABI_MISMATCH,
            );
            assert!(writer.is_null());
            assert!(error.is_null());
        };

        let mut options = WriteOptionsV1 {
            struct_size: size_of::<WriteOptionsV1>() as u32,
            struct_version: STRUCT_VERSION,
            present_bits: 1 << 63,
            flags: 0,
            content_type: absent,
            content_disposition: absent,
            content_encoding: absent,
            cache_control: absent,
            if_match: absent,
            if_none_match: absent,
        };
        assert_abi_mismatch(&options);
        options.present_bits = 0;
        options.flags = 1 << 63;
        assert_abi_mismatch(&options);
        options.flags = 0;
        options.content_type = bytes(b"noncanonical-absent");
        assert_abi_mismatch(&options);
        options.content_type = absent;
        options.struct_version = STRUCT_VERSION + 1;
        assert_abi_mismatch(&options);

        let invalid_utf8 = [0xFF];
        options.struct_version = STRUCT_VERSION;
        options.present_bits = WRITE_CONTENT_TYPE_PRESENT;
        options.content_type = bytes(&invalid_utf8);
        let mut invalid_writer = NonNull::<WriterV1>::dangling().as_ptr();
        let mut invalid_error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the present text is readable but intentionally invalid UTF-8.
        assert_eq!(
            unsafe {
                constructor(
                    operator,
                    &path,
                    &options,
                    &mut invalid_writer,
                    &mut invalid_error,
                )
            },
            STATUS_ERROR,
        );
        assert!(invalid_writer.is_null());
        assert_eq!(take_error_kind(&api, invalid_error), ERROR_INVALID_ARGUMENT);

        let writer = memory_writer(&api, operator, b"writer/input.bin", ptr::null());
        let malformed = BytesViewV1 {
            data: ptr::null(),
            len: 1,
        };
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the malformed carrier is readable and should not enter Writer state.
        assert_eq!(
            unsafe {
                api.writer_write.expect("CHUNKED_WRITER write is installed")(
                    writer, &malformed, &mut error,
                )
            },
            STATUS_ABI_MISMATCH,
        );
        assert!(error.is_null());
        write_writer_chunk(&api, writer, b"still-open");
        let metadata = finish_writer(&api, writer);
        assert_eq!(take_metadata_content_length(&api, metadata), 10);

        // SAFETY: all resources remain uniquely owned.
        unsafe {
            api.writer_free.expect("CHUNKED_WRITER free is installed")(writer);
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn chunked_writer_error_panic_and_poison_are_terminal() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let write = api.writer_write.expect("CHUNKED_WRITER write is installed");
        let close = api.writer_close.expect("CHUNKED_WRITER close is installed");
        let free = api.writer_free.expect("CHUNKED_WRITER free is installed");
        let data = bytes(b"value");

        let write_error = memory_writer(&api, operator, b"writer/write-error", ptr::null());
        install_writer_call_test_mode(write_error, TEST_WRITER_WRITE_ERROR);
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the test hook injects one ordinary native write failure.
        assert_eq!(
            unsafe { write(write_error, &data, &mut error) },
            STATUS_ERROR,
        );
        assert_eq!(take_error_kind(&api, error), ERROR_UNEXPECTED);
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the failed handle remains live but terminal.
        assert_eq!(
            unsafe { write(write_error, &data, &mut error) },
            STATUS_ERROR,
        );
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        let write_panic = memory_writer(&api, operator, b"writer/write-panic", ptr::null());
        install_writer_call_test_mode(write_panic, TEST_WRITER_WRITE_PANIC);
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the injected panic is contained at the ABI boundary.
        assert_eq!(
            unsafe { write(write_panic, &data, &mut error) },
            STATUS_PANIC,
        );
        assert!(error.is_null());
        let mut metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: a contained panic left the handle terminal.
        assert_eq!(
            unsafe { close(write_panic, &mut metadata, &mut error) },
            STATUS_ERROR,
        );
        assert!(metadata.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        let close_error = memory_writer(&api, operator, b"writer/close-error", ptr::null());
        write_writer_chunk(&api, close_error, b"value");
        install_writer_call_test_mode(close_error, TEST_WRITER_CLOSE_ERROR);
        metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the first close attempt injects an ordinary failure.
        assert_eq!(
            unsafe { close(close_error, &mut metadata, &mut error) },
            STATUS_ERROR,
        );
        assert!(metadata.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_UNEXPECTED);
        metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: every later close observes the terminal state.
        assert_eq!(
            unsafe { close(close_error, &mut metadata, &mut error) },
            STATUS_ERROR,
        );
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        let close_panic = memory_writer(&api, operator, b"writer/close-panic", ptr::null());
        install_writer_call_test_mode(close_panic, TEST_WRITER_CLOSE_PANIC);
        metadata = NonNull::<MetadataV1>::dangling().as_ptr();
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the injected close panic is contained and terminal.
        assert_eq!(
            unsafe { close(close_panic, &mut metadata, &mut error) },
            STATUS_PANIC,
        );
        assert!(metadata.is_null());
        assert!(error.is_null());
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the terminal handle rejects later writes.
        assert_eq!(
            unsafe { write(close_panic, &data, &mut error) },
            STATUS_ERROR,
        );
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        let poisoned_writer = memory_writer(&api, operator, b"writer/poison", ptr::null());
        let poisoned = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: this test owns the live handle and intentionally poisons its mutex.
            let writer = unsafe { &*poisoned_writer };
            let _guard = writer.state.lock().expect("fresh writer lock is healthy");
            panic!("intentionally poison writer state");
        }));
        assert!(poisoned.is_err());
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: poison is reported once while transitioning to Failed.
        assert_eq!(
            unsafe { write(poisoned_writer, &data, &mut error) },
            STATUS_ERROR,
        );
        assert_eq!(take_error_kind(&api, error), ERROR_UNEXPECTED);
        error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: the cleared mutex now exposes the deterministic terminal state.
        assert_eq!(
            unsafe { write(poisoned_writer, &data, &mut error) },
            STATUS_ERROR,
        );
        assert_eq!(take_error_kind(&api, error), ERROR_RESOURCE_CLOSED);

        // SAFETY: all handles and constructor outputs remain uniquely owned.
        unsafe {
            free(write_error);
            free(write_panic);
            free(close_error);
            free(close_panic);
            free(poisoned_writer);
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn buffer_copy_is_atomic_for_sizing_errors_and_success_tail() {
        let api = api();
        let copy = api.buffer_copy.expect("BASE buffer copy is installed");
        let free = api.buffer_free.expect("BASE buffer free is installed");
        let buffer = Box::into_raw(Box::new(BufferV1 {
            bytes: b"abcdef".to_vec(),
        }));
        let mut destination = [0xA5; 8];
        let mut required = u64::MAX;

        // SAFETY: buffer and output regions are live and non-overlapping.
        assert_eq!(
            unsafe { copy(buffer, destination.as_mut_ptr(), 5, &mut required) },
            STATUS_BUFFER_TOO_SMALL,
        );
        assert_eq!(required, 6);
        assert_eq!(destination, [0xA5; 8]);

        required = u64::MAX;
        // SAFETY: NULL with nonzero capacity is intentionally malformed.
        assert_eq!(
            unsafe { copy(buffer, ptr::null_mut(), 1, &mut required) },
            STATUS_ABI_MISMATCH,
        );
        assert_eq!(required, 0);
        assert_eq!(destination, [0xA5; 8]);

        required = u64::MAX;
        // SAFETY: the NULL handle is deliberately invalid; destination is valid.
        assert_eq!(
            unsafe {
                copy(
                    ptr::null(),
                    destination.as_mut_ptr(),
                    destination.len() as u64,
                    &mut required,
                )
            },
            STATUS_ABI_MISMATCH,
        );
        assert_eq!(required, 0);
        assert_eq!(destination, [0xA5; 8]);

        // SAFETY: capacity covers the destination and the immutable buffer.
        assert_eq!(
            unsafe {
                copy(
                    buffer,
                    destination.as_mut_ptr(),
                    destination.len() as u64,
                    &mut required,
                )
            },
            STATUS_OK,
        );
        assert_eq!(required, 6);
        assert_eq!(&destination[..6], b"abcdef");
        assert_eq!(&destination[6..], &[0xA5; 2]);

        let empty = Box::into_raw(Box::new(BufferV1 { bytes: Vec::new() }));
        required = u64::MAX;
        // SAFETY: NULL+zero is the documented sizing query for an empty buffer.
        assert_eq!(
            unsafe { copy(empty, ptr::null_mut(), 0, &mut required) },
            STATUS_OK,
        );
        assert_eq!(required, 0);
        // SAFETY: both handles are still uniquely owned by this test.
        unsafe {
            free(empty);
            free(buffer);
        }
    }

    #[test]
    fn presigned_snapshot_preserves_duplicate_binary_headers_and_borrowed_views() {
        let api = api();
        let mut headers = http::HeaderMap::new();
        headers.append("x-repeat", http::HeaderValue::from_static("first"));
        headers.append(
            "x-repeat",
            http::HeaderValue::from_bytes(&[0x80, b'z']).expect("binary header is valid"),
        );
        let request = PresignedRequest::new(
            http::Method::GET,
            "https://example.invalid/object?signature=secret"
                .parse()
                .expect("test URI is valid"),
            headers,
        );
        let snapshot = match checked_presigned_request(request) {
            Ok(snapshot) => snapshot,
            Err(_) => panic!("snapshot is within bounds"),
        };
        let request = Box::into_raw(Box::new(snapshot));
        let mut request_view = PresignedRequestViewV1 {
            struct_size: size_of::<PresignedRequestViewV1>() as u32,
            struct_version: STRUCT_VERSION,
            reserved0: u64::MAX,
            method: bytes(b"poison"),
            uri: bytes(b"poison"),
            header_count: u64::MAX,
        };
        // SAFETY: the request handle and complete output view are live.
        assert_eq!(
            unsafe {
                api.presigned_request_view
                    .expect("PRESIGN request view is installed")(
                    request, &mut request_view
                )
            },
            STATUS_OK,
        );
        assert_eq!(copy_view(request_view.method), b"GET");
        assert_eq!(
            copy_view(request_view.uri),
            b"https://example.invalid/object?signature=secret"
        );
        assert_eq!(request_view.header_count, 2);

        let mut observed = Vec::new();
        for index in 0..request_view.header_count {
            let mut header_view = PresignedHeaderViewV1 {
                struct_size: size_of::<PresignedHeaderViewV1>() as u32,
                struct_version: STRUCT_VERSION,
                reserved0: u64::MAX,
                name: bytes(b"poison"),
                value: bytes(b"poison"),
            };
            // SAFETY: the request handle and complete output view are live.
            assert_eq!(
                unsafe {
                    api.presigned_request_header_view
                        .expect("PRESIGN header view is installed")(
                        request,
                        index,
                        &mut header_view,
                    )
                },
                STATUS_OK,
            );
            observed.push((copy_view(header_view.name), copy_view(header_view.value)));
        }
        assert_eq!(
            observed,
            vec![
                (b"x-repeat".to_vec(), b"first".to_vec()),
                (b"x-repeat".to_vec(), vec![0x80, b'z']),
            ]
        );

        let mut outside = PresignedHeaderViewV1 {
            struct_size: size_of::<PresignedHeaderViewV1>() as u32,
            struct_version: STRUCT_VERSION,
            reserved0: u64::MAX,
            name: bytes(b"poison"),
            value: bytes(b"poison"),
        };
        // SAFETY: the request handle and complete output view are live.
        assert_eq!(
            unsafe {
                api.presigned_request_header_view
                    .expect("PRESIGN header view is installed")(
                    request,
                    request_view.header_count,
                    &mut outside,
                )
            },
            STATUS_END,
        );
        assert_eq!(outside.name.len, 0);
        assert_eq!(outside.value.len, 0);
        // SAFETY: this test owns the request handle exactly once.
        unsafe {
            api.presigned_request_free
                .expect("PRESIGN free is installed")(request)
        };
    }

    #[test]
    fn presign_validates_expiry_atomically_and_maps_capabilities() {
        let api = api();
        let (operator, info) = memory_operator(&api);
        let path = bytes(b"object");
        let mut request = NonNull::<PresignedRequestV1>::dangling().as_ptr();
        let mut error = NonNull::<ErrorV1>::dangling().as_ptr();
        // SAFETY: all carriers and output slots are valid for this call.
        let status = unsafe {
            api.operator_presign_read
                .expect("PRESIGN read is installed")(
                operator,
                &path,
                ptr::null(),
                0,
                &mut request,
                &mut error,
            )
        };
        assert_eq!(status, STATUS_ERROR);
        assert!(request.is_null());
        assert_eq!(take_error_kind(&api, error), ERROR_INVALID_ARGUMENT);

        let capability = Capability {
            presign_stat: true,
            presign_read: true,
            presign_write: true,
            ..Capability::default()
        };
        assert_eq!(
            capability_view(capability).words[0]
                & (CAP_PRESIGN_STAT | CAP_PRESIGN_READ | CAP_PRESIGN_WRITE),
            CAP_PRESIGN_STAT | CAP_PRESIGN_READ | CAP_PRESIGN_WRITE,
        );
        // SAFETY: this test uniquely owns both handles.
        unsafe {
            api.operator_info_free.expect("BASE info free is installed")(info);
            api.operator_free.expect("BASE operator free is installed")(operator);
        }
    }

    #[test]
    fn abi_thread_promises_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OperatorV1>();
        assert_send_sync::<BufferV1>();
        assert_send_sync::<ErrorV1>();
        assert_send_sync::<MetadataV1>();
        assert_send_sync::<EntryV1>();
        assert_send_sync::<OperatorInfoV1>();
        assert_send_sync::<ListerV1>();
        assert_send_sync::<ReaderV1>();
        assert_send_sync::<WriterV1>();
        assert_send_sync::<ReadStreamV1>();
        assert_send_sync::<PresignedRequestV1>();
    }
}
