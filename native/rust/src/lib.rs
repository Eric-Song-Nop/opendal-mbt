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
use std::sync::OnceLock;

use abi::*;
use opendal::options::{DeleteOptions, ReadOptions, StatOptions, WriteOptions};
use opendal::{BytesRange, Capability, EntryMode, ErrorKind, Metadata};
use tokio::runtime::Runtime;

const MAX_OUTPUT_BYTES: u64 = i32::MAX as u64;
const BINDING_VERSION: &str = env!("CARGO_PKG_VERSION");
const OPENDAL_VERSION: &str = "0.58.1";
const SERVICE_PROFILE: &str = "memory,fs";

static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();

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
        opendal::init_default_registry();
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
    CapabilityV1 {
        words: [word0, 0, 0, 0],
    }
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
        if let Err(failure) = unsafe { clear_required_output(out_operator, ptr::null_mut()) }
            .and_then(|_| unsafe { clear_required_output(out_info, ptr::null_mut()) })
            .and_then(|_| unsafe { clear_error_output(out_error) })
        {
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
            let operator =
                opendal::blocking::Operator::new(async_operator).map_err(construction_error)?;
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
            Ok((Box::new(OperatorV1 { inner: operator }), Box::new(info)))
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

unsafe extern "C" fn operator_free(operator: *mut OperatorV1) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !operator.is_null() {
            // SAFETY: every non-null handle is uniquely owned and freed once.
            unsafe { drop(Box::from_raw(operator)) };
        }
    }));
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
        if let Err(failure) = unsafe { clear_required_output(out_exists, 0) }
            .and_then(|_| unsafe { clear_error_output(out_error) })
        {
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
        if let Err(failure) = unsafe { clear_required_output(out_metadata, ptr::null_mut()) }
            .and_then(|_| unsafe { clear_error_output(out_error) })
        {
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
        if let Err(failure) = unsafe { clear_required_output(out_buffer, ptr::null_mut()) }
            .and_then(|_| unsafe { clear_error_output(out_error) })
        {
            return unsafe { finish_failure(failure, out_error) };
        }
        let result = (|| -> CallResult<Box<BufferV1>> {
            // SAFETY: handle/text/options are validated and copied as needed.
            let operator = unsafe { borrow_required(operator.cast_const())? };
            let path = unsafe { read_text(path, "path")? };
            let options = unsafe { parse_read_options(options)? };
            let buffer = operator
                .inner
                .read_options(&path, options)
                .map_err(opendal_error)?;
            let length = u64::try_from(buffer.len())
                .map_err(|_| buffer_too_large("read result length is not representable"))?;
            if length > MAX_OUTPUT_BYTES || length > max_output_len {
                return Err(buffer_too_large(
                    "read result exceeds the negotiated output limit",
                ));
            }
            Ok(Box::new(BufferV1 {
                bytes: buffer.to_vec(),
            }))
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
        if let Err(failure) = unsafe { clear_required_output(out_metadata, ptr::null_mut()) }
            .and_then(|_| unsafe { clear_error_output(out_error) })
        {
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
        if let Err(failure) = unsafe { clear_required_output(out_metadata, ptr::null_mut()) }
            .and_then(|_| unsafe { clear_error_output(out_error) })
        {
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

fn stage_api() -> Option<ApiV1> {
    Some(ApiV1 {
        struct_size: 0,
        requested_major: 0,
        library_struct_size: u32::try_from(size_of::<ApiV1>()).ok()?,
        library_minor: ABI_MINOR,
        library_patch: ABI_PATCH,
        reserved0: 0,
        feature_bits: FEATURE_BASE | FEATURE_WHOLE_OBJECT,
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
        operator_lister: None,
        lister_next: None,
        lister_close: None,
        lister_free: None,
        operator_reader: None,
        reader_read: None,
        reader_close: None,
        reader_free: None,
        operator_writer: None,
        writer_write: None,
        writer_close: None,
        writer_free: None,
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
    // Disabled groups are intentionally left zero by the bounded clear.
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

    #[test]
    fn bootstrap_installs_complete_supported_groups() {
        let api = api();
        assert_eq!(api.library_struct_size as usize, size_of::<ApiV1>());
        assert_eq!(api.feature_bits, FEATURE_BASE | FEATURE_WHOLE_OBJECT);
        assert!(api.library_info.is_some());
        assert!(api.operator_new.is_some());
        assert!(api.operator_rename.is_some());
        assert!(api.operator_lister.is_none());
        assert!(api.operator_reader.is_none());
        assert!(api.operator_writer.is_none());
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
    fn abi_thread_promises_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OperatorV1>();
        assert_send_sync::<BufferV1>();
        assert_send_sync::<ErrorV1>();
        assert_send_sync::<MetadataV1>();
        assert_send_sync::<EntryV1>();
        assert_send_sync::<OperatorInfoV1>();
    }
}
