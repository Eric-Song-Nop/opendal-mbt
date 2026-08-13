//! Scalar-only WebAssembly bridge between MoonBit and OpenDAL.
//!
//! This crate is an integration canary. It owns all OpenDAL and byte-buffer
//! values behind generation-checked handles so neither Rust pointers nor
//! language-specific object layouts become part of the module ABI.

use std::cell::RefCell;
use std::future::Future;
use std::pin::{Pin, pin};
use std::task::{Context, Poll, Waker};

use futures_util::TryStreamExt;
use opendal::{ErrorKind, Metadata, Operator, OperatorRegistry, services::Memory};

const ABI_VERSION: u32 = 0x0001_0004;
const FEATURE_MEMORY_SERVICE: u32 = 1 << 0;
const FEATURE_POLL_ONCE_CANARY: u32 = 1 << 1;
const FEATURE_GENERATION_HANDLES: u32 = 1 << 2;
const FEATURE_BINARY_BUFFERS: u32 = 1 << 3;
const FEATURE_TASK_ABI: u32 = 1 << 4;
const FEATURE_GENERIC_OPERATOR: u32 = 1 << 5;
const FEATURE_CORE_MUTATIONS: u32 = 1 << 6;
const FEATURE_BOUNDED_LIST: u32 = 1 << 7;
const FEATURE_BULK_TRANSFER: u32 = 1 << 8;
const MAX_SLOTS: usize = u16::MAX as usize;
const MAX_GENERATION: u16 = i16::MAX as u16;
const MAX_BUFFER_LENGTH: usize = 64 * 1024 * 1024;
const MAX_TRANSFER_CHUNK: usize = 256 * 1024;
const MAX_LIST_ENTRIES: usize = 65_536;
const MAX_LIST_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

const STATUS_OK: u32 = 0;
const SCALAR_ERROR: i32 = -1;

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
    #[error("task {handle} is not ready")]
    TaskNotReady { handle: u32 },
    #[error("task {handle} has already been consumed")]
    TaskConsumed { handle: u32 },
    #[error("the bridge instance has been torn down")]
    TornDown,
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
            Self::TaskNotReady { .. } => 12,
            Self::TaskConsumed { .. } => 13,
            Self::TornDown => 14,
            Self::ListTooLarge => 15,
            Self::AllocationFailed => 16,
            Self::InvalidArgument { .. } => 17,
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
    OperatorBuilder,
    Operator,
    Metadata,
    Error,
    Task,
    Completion,
    EntryList,
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
        }
    }
}

#[derive(Clone)]
struct OperatorBuilder {
    scheme: String,
    config: Vec<(String, String)>,
}

struct EntrySnapshot {
    path: String,
    name: String,
    mode: u32,
    content_length: u64,
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
        }
    }
}

enum Task {
    Pending,
    Ready(Box<Completion>),
    Cancelled,
    Consumed,
}

enum Completion {
    Write(Result<(), BridgeError>),
    Read(Result<Vec<u8>, BridgeError>),
    Stat(Result<Box<Metadata>, BridgeError>),
    CreateDir(Result<(), BridgeError>),
    Delete(Result<(), BridgeError>),
    List(Result<Vec<EntrySnapshot>, BridgeError>),
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
            Self::Write(Ok(()))
            | Self::Read(Ok(_))
            | Self::Stat(Ok(_))
            | Self::CreateDir(Ok(()))
            | Self::Delete(Ok(()))
            | Self::List(Ok(_)) => None,
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
    forced_pending_poll_count: u32,
    force_pending_for_canary: bool,
    torn_down: bool,
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
    let handle = insert_resource(Resource::Task(Task::Pending))?;
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
            && matches!(task, Task::Pending)
        {
            *task = Task::Ready(Box::new(completion));
        }
    });
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

fn owned_utf8_buffer(handle: u32) -> Result<String, BridgeError> {
    STATE.with(|state| {
        let state = state.borrow();
        std::str::from_utf8(state.arena.buffer(handle)?)
            .map(str::to_owned)
            .map_err(|_| BridgeError::InvalidUtf8)
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
    path: &str,
    name: &str,
    mode: u32,
    content_length: u64,
    bounds: ListBounds,
) -> Result<(), BridgeError> {
    let next_bytes = total_bytes
        .checked_add(path.len())
        .and_then(|value| value.checked_add(name.len()))
        .ok_or(BridgeError::ListTooLarge)?;
    if entries.len() >= bounds.max_entries || next_bytes > bounds.max_bytes {
        return Err(BridgeError::ListTooLarge);
    }
    entries
        .try_reserve(1)
        .map_err(|_| BridgeError::AllocationFailed)?;
    entries.push(EntrySnapshot {
        path: try_owned_string(path)?,
        name: try_owned_string(name)?,
        mode,
        content_length,
    });
    *total_bytes = next_bytes;
    Ok(())
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
        let metadata = entry.metadata();
        let mode = match metadata.mode() {
            opendal::EntryMode::FILE => ENTRY_MODE_FILE,
            opendal::EntryMode::DIR => ENTRY_MODE_DIRECTORY,
            opendal::EntryMode::Unknown => ENTRY_MODE_UNKNOWN,
        };
        push_entry_snapshot(
            &mut entries,
            &mut total_bytes,
            entry.path(),
            entry.name(),
            mode,
            metadata.content_length(),
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

fn build_operator(builder: &OperatorBuilder) -> Result<Operator, BridgeError> {
    opendal::init_default_registry();
    Operator::via_iter(&builder.scheme, builder.config.clone()).map_err(BridgeError::from)
}

fn capability_word(operator: &Operator, word: u32) -> Result<u64, BridgeError> {
    let capability = operator.info().capability();
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
            value |= u64::from(capability.read_with_suffix) * CAP_READ_SUFFIX;
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
            return Err(BridgeError::OpenDal {
                message: "service scheme cannot be empty".to_owned(),
            });
        }
        insert_resource(Resource::OperatorBuilder(OperatorBuilder {
            scheme,
            config: Vec::new(),
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
            state
                .borrow_mut()
                .arena
                .operator_builder_mut(builder_handle)?
                .config
                .push((key, value));
            Ok(())
        })
    });
    status(result)
}

/// Builds an operator through OpenDAL's compiled service registry.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_builder_build(builder_handle: u32) -> u32 {
    let result = STATE.with(|state| {
        Ok(state
            .borrow()
            .arena
            .operator_builder(builder_handle)?
            .clone())
    });
    handle_or_record_error(result.and_then(|builder| {
        build_operator(&builder).and_then(|operator| insert_resource(Resource::Operator(operator)))
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
            .map_err(|_| BridgeError::InvalidUtf8)?
            .to_owned();
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
    let inputs = STATE.with(|state| {
        let state = state.borrow();
        let operator = state.arena.operator(operator_handle)?.clone();
        let path = std::str::from_utf8(state.arena.buffer(path_handle)?)
            .map_err(|_| BridgeError::InvalidUtf8)?
            .to_owned();
        let data = try_owned_bytes(state.arena.buffer(data_handle)?)?;
        Ok((operator, path, data))
    });
    let result = inputs.and_then(|(operator, path, data)| {
        start_task(async move {
            Completion::Write(
                operator
                    .write(&path, data)
                    .await
                    .map(|_| ())
                    .map_err(BridgeError::from),
            )
        })
    });
    handle_or_record_error(result)
}

/// Starts an asynchronous whole-object read and returns an owned task handle.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_read_start(
    operator_handle: u32,
    path_handle: u32,
) -> u32 {
    let result = path_and_operator(operator_handle, path_handle).and_then(|(operator, path)| {
        start_task(async move {
            Completion::Read(match operator.read(&path).await {
                Ok(buffer) => try_owned_opendal_buffer(buffer),
                Err(error) => Err(BridgeError::from(error)),
            })
        })
    });
    handle_or_record_error(result)
}

/// Starts an asynchronous metadata lookup and returns an owned task handle.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_stat_start(
    operator_handle: u32,
    path_handle: u32,
) -> u32 {
    let result = path_and_operator(operator_handle, path_handle).and_then(|(operator, path)| {
        start_task(async move {
            Completion::Stat(
                operator
                    .stat(&path)
                    .await
                    .map(Box::new)
                    .map_err(BridgeError::from),
            )
        })
    });
    handle_or_record_error(result)
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
            Task::Pending => TASK_PENDING,
            Task::Ready(_) => TASK_READY,
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
            Task::Ready(completion) => state.arena.insert(Resource::Completion(*completion)),
            Task::Pending => {
                *task = Task::Pending;
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
        let task = state.arena.task_mut(handle)?;
        match task {
            Task::Pending | Task::Ready(_) => *task = Task::Cancelled,
            Task::Cancelled | Task::Consumed => {}
        }
        Ok(())
    });
    status(result)
}

/// Releases a task and any unconsumed completion it still owns.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_task_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::Task)
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
        })
    });
    match result {
        Ok(completion_status) => completion_status,
        Err(error) => record_error(error),
    }
}

/// Moves a successful read result into a buffer handle, or returns `0`.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_buffer(handle: u32) -> u32 {
    let result = take_successful_completion(handle, COMPLETION_READ).and_then(|completion| {
        let Completion::Read(Ok(buffer)) = completion else {
            unreachable!("completion kind and status checked before removal")
        };
        insert_buffer(buffer)
    });
    handle_or_record_error(result)
}

/// Moves a successful stat result into a metadata handle, or returns `0`.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_metadata(handle: u32) -> u32 {
    let result = take_successful_completion(handle, COMPLETION_STAT).and_then(|completion| {
        let Completion::Stat(Ok(metadata)) = completion else {
            unreachable!("completion kind and status checked before removal")
        };
        STATE.with(|state| {
            state
                .borrow_mut()
                .arena
                .insert(Resource::Metadata(*metadata))
        })
    });
    handle_or_record_error(result)
}

/// Moves a successful list result into an owned entry-list handle.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_entry_list(handle: u32) -> u32 {
    let result = take_successful_completion(handle, COMPLETION_LIST).and_then(|completion| {
        let Completion::List(Ok(entries)) = completion else {
            unreachable!("completion kind and status checked before removal")
        };
        STATE.with(|state| {
            state
                .borrow_mut()
                .arena
                .insert(Resource::EntryList(entries))
        })
    });
    handle_or_record_error(result)
}

fn take_successful_completion(handle: u32, expected_kind: u32) -> Result<Completion, BridgeError> {
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
        state
            .arena
            .ensure_insert_capacity_after_take(handle, ResourceKind::Completion)?;
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
            | Completion::List(Err(error)) => error,
            Completion::Write(Ok(()))
            | Completion::Read(Ok(_))
            | Completion::Stat(Ok(_))
            | Completion::CreateDir(Ok(()))
            | Completion::Delete(Ok(()))
            | Completion::List(Ok(_)) => unreachable!("success checked before removal"),
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

/// Releases an owned error handle and returns a status code.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_error_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::Error)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(take_test_buffer(schemes), b"memory");

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
    fn generic_builder_reports_an_unregistered_scheme_without_leaking() {
        reset_state();
        let scheme = insert_test_buffer(b"not-compiled-in");
        let builder = opendal_mbt_wasm_operator_builder_new(scheme);
        assert_ne!(builder, 0);
        assert_eq!(opendal_mbt_wasm_buffer_release(scheme), STATUS_OK);

        assert_eq!(opendal_mbt_wasm_operator_builder_build(builder), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 8);
        let error = opendal_mbt_wasm_last_error_take();
        assert_ne!(error, 0);
        let message = opendal_mbt_wasm_error_message(error);
        assert!(
            String::from_utf8(take_test_buffer(message))
                .unwrap()
                .contains("scheme is not registered")
        );
        assert_eq!(opendal_mbt_wasm_error_release(error), STATUS_OK);
        assert_eq!(
            opendal_mbt_wasm_operator_builder_release(builder),
            STATUS_OK
        );
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn unit_mutation_completions_are_typed_and_releasable() {
        reset_state();
        for (completion, expected_kind) in [
            (Completion::CreateDir(Ok(())), COMPLETION_CREATE_DIR),
            (Completion::Delete(Ok(())), COMPLETION_DELETE),
        ] {
            let task = insert_test_resource(Resource::Task(Task::Ready(Box::new(completion))));
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
            max_bytes: 5,
        };
        assert!(
            push_entry_snapshot(
                &mut entries,
                &mut bytes,
                "a",
                "a",
                ENTRY_MODE_FILE,
                1,
                bounds,
            )
            .is_ok()
        );
        assert!(
            push_entry_snapshot(
                &mut entries,
                &mut bytes,
                "b",
                "b",
                ENTRY_MODE_FILE,
                1,
                bounds,
            )
            .is_ok()
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(bytes, 4);

        assert!(matches!(
            push_entry_snapshot(
                &mut entries,
                &mut bytes,
                "c",
                "c",
                ENTRY_MODE_FILE,
                1,
                bounds,
            ),
            Err(BridgeError::ListTooLarge)
        ));
        assert_eq!(entries.len(), 2);
        assert_eq!(bytes, 4);

        let mut bytes_limited = Vec::new();
        let mut byte_count = 0;
        assert!(matches!(
            push_entry_snapshot(
                &mut bytes_limited,
                &mut byte_count,
                "abc",
                "def",
                ENTRY_MODE_FILE,
                3,
                bounds,
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
        assert_eq!(opendal_mbt_wasm_entry_list_mode(entries, 1), SCALAR_ERROR);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 4);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_entry_list_release(entries), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn ready_task_moves_its_completion_exactly_once() {
        reset_state();
        let task = insert_test_resource(Resource::Task(Task::Ready(Box::new(Completion::Read(
            Ok(vec![0, 255, 42]),
        )))));

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
    fn failed_result_remains_available_for_owned_error_take() {
        reset_state();
        let completion = insert_test_resource(Resource::Completion(Completion::Read(Err(
            BridgeError::OpenDalNotFound {
                message: "missing".to_owned(),
            },
        ))));

        assert_eq!(opendal_mbt_wasm_completion_take_buffer(completion), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 7);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);

        let error = opendal_mbt_wasm_completion_take_error(completion);
        assert_ne!(error, 0);
        assert_eq!(opendal_mbt_wasm_error_code(error), 7);
        assert_eq!(opendal_mbt_wasm_error_release(error), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn cancellation_and_release_make_late_completion_inert() {
        reset_state();
        let stale = insert_test_resource(Resource::Task(Task::Pending));

        assert_eq!(opendal_mbt_wasm_task_cancel(stale), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_task_state(stale), TASK_CANCELLED);
        publish_task(stale, Completion::Write(Ok(())));
        assert_eq!(opendal_mbt_wasm_task_state(stale), TASK_CANCELLED);
        assert_eq!(opendal_mbt_wasm_task_release(stale), STATUS_OK);

        let current = opendal_mbt_wasm_buffer_new();
        assert_ne!(current, 0);
        assert_ne!(current, stale);
        publish_task(stale, Completion::Write(Ok(())));
        assert_eq!(opendal_mbt_wasm_buffer_len(current), 0);
        assert_eq!(opendal_mbt_wasm_buffer_release(current), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn cancellation_wins_over_an_unclaimed_ready_result() {
        reset_state();
        let task = insert_test_resource(Resource::Task(Task::Ready(Box::new(Completion::Write(
            Ok(()),
        )))));

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
        let task = insert_test_resource(Resource::Task(Task::Pending));

        assert_eq!(opendal_mbt_wasm_teardown(), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_teardown(), STATUS_OK);
        publish_task(task, Completion::Write(Ok(())));
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);

        assert_eq!(opendal_mbt_wasm_buffer_new(), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 14);
        assert_eq!(opendal_mbt_wasm_last_error_take(), 0);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }
}
