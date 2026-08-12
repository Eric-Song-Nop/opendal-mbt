//! Scalar-only WebAssembly bridge between MoonBit and OpenDAL.
//!
//! This crate is an integration canary. It owns all OpenDAL and byte-buffer
//! values behind generation-checked handles so neither Rust pointers nor
//! language-specific object layouts become part of the module ABI.

use std::cell::RefCell;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use opendal::{ErrorKind, Metadata, Operator, services::Memory};

const ABI_VERSION: u32 = 0x0001_0000;
const FEATURE_MEMORY_SERVICE: u32 = 1 << 0;
const FEATURE_POLL_ONCE_CANARY: u32 = 1 << 1;
const FEATURE_GENERATION_HANDLES: u32 = 1 << 2;
const FEATURE_BINARY_BUFFERS: u32 = 1 << 3;
const MAX_SLOTS: usize = u16::MAX as usize;
const MAX_GENERATION: u16 = i16::MAX as u16;
const MAX_BUFFER_LENGTH: usize = i32::MAX as usize;

const STATUS_OK: u32 = 0;
const SCALAR_ERROR: i32 = -1;

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
    #[error("OpenDAL path was not found: {message}")]
    OpenDalNotFound { message: String },
    #[error("OpenDAL operation failed: {message}")]
    OpenDal { message: String },
    #[error("OpenDAL future returned Pending in the poll-once canary adapter")]
    AsyncPending,
    #[error("the bridge has reached its {MAX_SLOTS}-handle capacity")]
    HandleLimit,
    #[error("value cannot be represented by the scalar ABI")]
    LengthOverflow,
}

impl BridgeError {
    fn code(&self) -> u32 {
        match self {
            Self::InvalidHandle { .. } => 1,
            Self::WrongResourceType { .. } => 2,
            Self::BufferTooLarge => 3,
            Self::IndexOutOfBounds { .. } => 4,
            Self::InvalidByte { .. } => 5,
            Self::InvalidUtf8 => 6,
            Self::OpenDalNotFound { .. } => 7,
            Self::OpenDal { .. } => 8,
            Self::AsyncPending => 9,
            Self::HandleLimit => 10,
            Self::LengthOverflow => 11,
        }
    }
}

impl From<opendal::Error> for BridgeError {
    fn from(error: opendal::Error) -> Self {
        if error.kind() == ErrorKind::NotFound {
            Self::OpenDalNotFound {
                message: error.to_string(),
            }
        } else {
            Self::OpenDal {
                message: error.to_string(),
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResourceKind {
    Buffer,
    Operator,
    Metadata,
    Error,
}

impl ResourceKind {
    fn name(self) -> &'static str {
        match self {
            Self::Buffer => "buffer",
            Self::Operator => "operator",
            Self::Metadata => "metadata",
            Self::Error => "error",
        }
    }
}

enum Resource {
    Buffer(Vec<u8>),
    Operator(Operator),
    Metadata(Metadata),
    Error(BridgeError),
}

impl Resource {
    fn kind(&self) -> ResourceKind {
        match self {
            Self::Buffer(_) => ResourceKind::Buffer,
            Self::Operator(_) => ResourceKind::Operator,
            Self::Metadata(_) => ResourceKind::Metadata,
            Self::Error(_) => ResourceKind::Error,
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

    fn release(&mut self, handle: u32, expected: ResourceKind) -> Result<(), BridgeError> {
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

        slot.resource = None;
        slot.generation = next_generation(slot.generation);
        self.live -= 1;
        Ok(())
    }
}

#[derive(Default)]
struct State {
    arena: Arena,
    last_error: Option<BridgeError>,
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

fn path_and_operator(
    operator_handle: u32,
    path_handle: u32,
) -> Result<(Operator, String), BridgeError> {
    STATE.with(|state| {
        let state = state.borrow();
        let operator = state.arena.operator(operator_handle)?.clone();
        let path = std::str::from_utf8(state.arena.buffer(path_handle)?)
            .map_err(|_| BridgeError::InvalidUtf8)?
            .to_owned();
        Ok((operator, path))
    })
}

fn insert_resource(resource: Resource) -> Result<u32, BridgeError> {
    STATE.with(|state| state.borrow_mut().arena.insert(resource))
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
}

/// Returns the number of resource handles that have not been released.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_live_handle_count() -> u32 {
    STATE.with(|state| state.borrow().arena.live)
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
    match insert_resource(Resource::Buffer(Vec::new())) {
        Ok(handle) => handle,
        Err(error) => {
            record_error(error);
            0
        }
    }
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
            .map_err(|_| BridgeError::InvalidUtf8)?
            .to_owned();
        let data = state.arena.buffer(data_handle)?.to_vec();
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
        insert_resource(Resource::Buffer(buffer.to_vec()))
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
        .and_then(|message| insert_resource(Resource::Buffer(message)));
    match result {
        Ok(message_handle) => message_handle,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Releases an owned error handle and returns a status code.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_error_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::Error)
}
