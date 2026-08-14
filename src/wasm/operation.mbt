///|
const COMPLETION_WRITE = 1U

///|
const COMPLETION_READ = 2U

///|
const COMPLETION_STAT = 3U

///|
const COMPLETION_CREATE_DIR = 4U

///|
const COMPLETION_DELETE = 5U

///|
const COMPLETION_LIST = 6U

///|
const COMPLETION_CHECK = 7U

///|
const COMPLETION_EXISTS = 8U

///|
const COMPLETION_COPY = 9U

///|
const COMPLETION_RENAME = 10U

///|
priv enum CompletionTake {
  CompletionLocalMismatch
  CompletionBridgeFailure
  CompletionTaken(UInt)
}

///|
/// Return the local state of this experimental callback task.
pub fn Task::state(self : Task) -> TaskState {
  self.state
}

///|
/// Request logical cancellation. Cancelling more than once is harmless.
///
/// Cancellation suppresses the user callback. The underlying browser I/O may
/// continue, but any late completion is discarded by the bridge.
pub fn Task::cancel(self : Task) -> Unit {
  if self.state is Pending && self.handle != 0U {
    host_cancel_wait(self.handle)
    ignore(bridge_task_cancel(self.handle))
    ignore(bridge_task_release(self.handle))
    self.handle = 0U
    self.state = Cancelled
  }
}

///|
/// Release this task without delivering a later callback.
///
/// Closing more than once is harmless.
pub fn Task::close(self : Task) -> Unit {
  if self.state is Pending {
    self.cancel()
    self.state = Closed
  } else if self.state is Cancelled || self.state is Completed {
    self.state = Closed
  }
}

///|
fn Task::take_completion(self : Task, task : UInt) -> CompletionTake {
  if self.state != Pending || self.handle != task {
    return CompletionLocalMismatch
  }
  let completion = bridge_task_take(task)
  ignore(bridge_task_release(task))
  self.handle = 0U
  self.state = Completed
  if completion == 0U {
    CompletionBridgeFailure
  } else {
    CompletionTaken(completion)
  }
}

///|
fn completion_error(
  completion : UInt,
  fallback : String,
  context : ErrorContext,
) -> OpenDalError {
  let error = bridge_completion_take_error(completion)
  if error == 0U {
    ignore(bridge_completion_release(completion))
    return take_bridge_error(Unexpected, fallback, context)
  }
  take_owned_bridge_error(error, Unexpected, fallback, context)
}

///|
fn start_wait(operation : Task, finish : (UInt) -> Unit) -> Unit {
  host_wait_task(operation.handle, finish)
}

///|
fn finish_unit_operation(
  operation : Task,
  task : UInt,
  expected_kind : UInt,
  name : String,
  context : ErrorContext,
  callback : (Result[Unit, OpenDalError]) -> Unit,
) -> Unit {
  match operation.take_completion(task) {
    CompletionLocalMismatch =>
      callback(
        Err(
          local_error(
            ResourceClosed,
            "\{name} task no longer owns the pending callback",
            context,
          ),
        ),
      )
    CompletionBridgeFailure =>
      callback(
        Err(
          take_bridge_error(
            ResourceClosed,
            "\{name} task became ready without a completion",
            context,
          ),
        ),
      )
    CompletionTaken(completion) => {
      let kind = bridge_completion_kind(completion)
      let status = bridge_completion_status(completion)
      if kind != expected_kind {
        ignore(bridge_completion_release(completion))
        callback(
          Err(
            local_error(
              AbiMismatch,
              "\{name} task returned the wrong completion kind",
              context,
            ),
          ),
        )
      } else if status != 0U {
        callback(Err(completion_error(completion, "\{name} failed", context)))
      } else {
        ignore(bridge_completion_release(completion))
        callback(Ok(()))
      }
    }
  }
}

///|
fn release_bridge_inputs(handles : Array[UInt]) -> Unit {
  for handle in handles {
    ignore(bridge_buffer_release(handle))
  }
}

///|
fn copy_bridge_input(
  value : StringView,
  handles : Array[UInt],
  context : ErrorContext,
) -> UInt raise OpenDalError {
  let handle = copy_path_to_bridge(value, context)
  handles.push(handle)
  handle
}

///|
fn copy_optional_bridge_input(
  value : StringView?,
  handles : Array[UInt],
  context : ErrorContext,
) -> UInt raise OpenDalError {
  match value {
    None => 0U
    Some(value) => copy_bridge_input(value, handles, context)
  }
}

///|
fn ByteRange::bridge_parts(
  self : ByteRange,
  capability : Capability,
  context : ErrorContext,
) -> (UInt, UInt64, UInt64) raise OpenDalError {
  match self {
    Full => (0U, 0UL, 0UL)
    From(offset~) => (1U, offset, 0UL)
    Range(offset~, length~) => {
      if length > 18446744073709551615UL - offset {
        raise local_error(
          InvalidArgument,
          "byte range offset plus length exceeds UInt64",
          context,
        )
      }
      (2U, offset, length)
    }
    Suffix(length~) => {
      if !capability.can_read_suffix() {
        raise local_error(
          Unsupported,
          "suffix reads require native backend range support",
          context,
        )
      }
      (3U, 0UL, length)
    }
  }
}

///|
fn take_metadata_completion(
  completion : UInt,
  context : ErrorContext,
) -> Metadata raise OpenDalError {
  let buffer = bridge_completion_take_metadata_snapshot(completion)
  if buffer == 0U {
    ignore(bridge_completion_release(completion))
    raise_last_bridge_error(
      Unexpected,
      "could not take the metadata result",
      context,
    )
  }
  let snapshot = copy_buffer_from_bridge(buffer, context) catch {
    error => {
      ignore(bridge_buffer_release(buffer))
      raise error
    }
  }
  ignore(bridge_buffer_release(buffer))
  parse_metadata_snapshot(snapshot, context)
}

///|
fn finish_metadata_operation(
  operation : Task,
  task : UInt,
  expected_kind : UInt,
  name : String,
  context : ErrorContext,
  callback : (Result[Metadata, OpenDalError]) -> Unit,
) -> Unit {
  match operation.take_completion(task) {
    CompletionLocalMismatch =>
      callback(
        Err(
          local_error(
            ResourceClosed,
            "\{name} task no longer owns the pending callback",
            context,
          ),
        ),
      )
    CompletionBridgeFailure =>
      callback(
        Err(
          take_bridge_error(
            ResourceClosed,
            "\{name} task became ready without a completion",
            context,
          ),
        ),
      )
    CompletionTaken(completion) => {
      let kind = bridge_completion_kind(completion)
      let status = bridge_completion_status(completion)
      if kind != expected_kind {
        ignore(bridge_completion_release(completion))
        callback(
          Err(
            local_error(
              AbiMismatch,
              "\{name} task returned the wrong completion kind",
              context,
            ),
          ),
        )
      } else if status != 0U {
        callback(Err(completion_error(completion, "\{name} failed", context)))
      } else {
        let result = try take_metadata_completion(completion, context) catch {
          error => Err(error)
        } noraise {
          metadata => Ok(metadata)
        }
        callback(result)
      }
    }
  }
}

///|
fn finish_bool_operation(
  operation : Task,
  task : UInt,
  expected_kind : UInt,
  name : String,
  context : ErrorContext,
  callback : (Result[Bool, OpenDalError]) -> Unit,
) -> Unit {
  match operation.take_completion(task) {
    CompletionLocalMismatch =>
      callback(
        Err(
          local_error(
            ResourceClosed,
            "\{name} task no longer owns the pending callback",
            context,
          ),
        ),
      )
    CompletionBridgeFailure =>
      callback(
        Err(
          take_bridge_error(
            ResourceClosed,
            "\{name} task became ready without a completion",
            context,
          ),
        ),
      )
    CompletionTaken(completion) => {
      let kind = bridge_completion_kind(completion)
      let status = bridge_completion_status(completion)
      if kind != expected_kind {
        ignore(bridge_completion_release(completion))
        callback(
          Err(
            local_error(
              AbiMismatch,
              "\{name} task returned the wrong completion kind",
              context,
            ),
          ),
        )
      } else if status != 0U {
        callback(Err(completion_error(completion, "\{name} failed", context)))
      } else {
        let value = bridge_completion_take_bool(completion)
        match value {
          0 => callback(Ok(false))
          1 => callback(Ok(true))
          -1 => {
            let error = take_bridge_error(
              Unexpected,
              "could not take the \{name} result",
              context,
            )
            ignore(bridge_completion_release(completion))
            callback(Err(error))
          }
          _ => {
            ignore(bridge_completion_release(completion))
            callback(
              Err(
                local_error(
                  AbiMismatch,
                  "\{name} task returned a non-canonical boolean",
                  context,
                ),
              ),
            )
          }
        }
      }
    }
  }
}

///|
/// Start a callback-based asynchronous connectivity check.
///
/// The callback is delivered from a later microtask or timer turn, never from
/// the initiating call stack.
pub fn AsyncOperator::check_callback(
  self : AsyncOperator,
  callback~ : (Result[Unit, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let context = error_context(Check)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let task = bridge_operator_check_start(self.native.handle)
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous check",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_unit_operation(
      operation,
      task,
      COMPLETION_CHECK,
      "check",
      context,
      callback,
    )
  })
  operation
}

///|
/// Start a callback-based asynchronous existence check.
///
/// Only NotFound becomes `false`; all other backend failures remain errors.
pub fn AsyncOperator::exists_callback(
  self : AsyncOperator,
  path : StringView,
  callback~ : (Result[Bool, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let path = path.to_owned()
  let context = error_context(Exists, path~)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let handles : Array[UInt] = []
  defer release_bridge_inputs(handles)
  let path_handle = copy_bridge_input(path, handles, context)
  let task = bridge_operator_exists_start(self.native.handle, path_handle)
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous exists",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_bool_operation(
      operation,
      task,
      COMPLETION_EXISTS,
      "exists",
      context,
      callback,
    )
  })
  operation
}

///|
/// Start a callback-based asynchronous write and return its metadata.
///
/// The callback is delivered from a later microtask or timer turn, never from
/// the initiating call stack.
pub fn AsyncOperator::write_callback(
  self : AsyncOperator,
  path : StringView,
  data : BytesView,
  append? : Bool = false,
  content_type? : StringView,
  content_disposition? : StringView,
  content_encoding? : StringView,
  cache_control? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
  callback~ : (Result[Metadata, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let path = path.to_owned()
  let context = error_context(Write, path~)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let handles : Array[UInt] = []
  defer release_bridge_inputs(handles)
  let path_handle = copy_bridge_input(path, handles, context)
  let data_handle = copy_bytes_to_bridge(data, context)
  handles.push(data_handle)
  let content_type_handle = copy_optional_bridge_input(
    content_type, handles, context,
  )
  let content_disposition_handle = copy_optional_bridge_input(
    content_disposition, handles, context,
  )
  let content_encoding_handle = copy_optional_bridge_input(
    content_encoding, handles, context,
  )
  let cache_control_handle = copy_optional_bridge_input(
    cache_control, handles, context,
  )
  let if_match_handle = copy_optional_bridge_input(if_match, handles, context)
  let if_none_match_handle = copy_optional_bridge_input(
    if_none_match, handles, context,
  )
  let task = bridge_operator_write_options_start_v1(
    self.native.handle,
    path_handle,
    data_handle,
    append,
    content_type_handle,
    content_disposition_handle,
    content_encoding_handle,
    cache_control_handle,
    if_match_handle,
    if_none_match_handle,
  )
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous write",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_metadata_operation(
      operation,
      task,
      COMPLETION_WRITE,
      "write",
      context,
      callback,
    )
  })
  operation
}

///|
/// Start a callback-based asynchronous whole-object or ranged read.
///
/// The callback is delivered from a later microtask or timer turn, never from
/// the initiating call stack.
pub fn AsyncOperator::read_callback(
  self : AsyncOperator,
  path : StringView,
  range? : ByteRange = Full,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
  callback~ : (Result[Bytes, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let path = path.to_owned()
  let context = error_context(Read, path~)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  let (range_kind, range_offset, range_length) = range.bridge_parts(
    self.native.info_snapshot.capability,
    context,
  )
  ignore(bridge_last_error_clear())
  let handles : Array[UInt] = []
  defer release_bridge_inputs(handles)
  let path_handle = copy_bridge_input(path, handles, context)
  let version_handle = copy_optional_bridge_input(version, handles, context)
  let if_match_handle = copy_optional_bridge_input(if_match, handles, context)
  let if_none_match_handle = copy_optional_bridge_input(
    if_none_match, handles, context,
  )
  let task = bridge_operator_read_options_start_v1(
    self.native.handle,
    path_handle,
    range_kind,
    range_offset,
    range_length,
    version_handle,
    if_match_handle,
    if_none_match_handle,
  )
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous read",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    match operation.take_completion(task) {
      CompletionLocalMismatch =>
        callback(
          Err(
            local_error(
              ResourceClosed,
              "read task no longer owns the pending callback",
              context,
            ),
          ),
        )
      CompletionBridgeFailure =>
        callback(
          Err(
            take_bridge_error(
              ResourceClosed,
              "read task became ready without a completion",
              context,
            ),
          ),
        )
      CompletionTaken(completion) => {
        let kind = bridge_completion_kind(completion)
        let status = bridge_completion_status(completion)
        if kind != COMPLETION_READ {
          ignore(bridge_completion_release(completion))
          callback(
            Err(
              local_error(
                AbiMismatch,
                "read task returned the wrong completion kind",
                context,
              ),
            ),
          )
        } else if status != 0U {
          callback(Err(completion_error(completion, "read failed", context)))
        } else {
          let buffer = bridge_completion_take_buffer(completion)
          if buffer == 0U {
            ignore(bridge_completion_release(completion))
            callback(
              Err(
                take_bridge_error(
                  Unexpected,
                  "could not take the read result",
                  context,
                ),
              ),
            )
          } else {
            let result = try copy_buffer_from_bridge(buffer, context) catch {
              error => Err(error)
            } noraise {
              bytes => Ok(bytes)
            }
            ignore(bridge_buffer_release(buffer))
            callback(result)
          }
        }
      }
    }
  })
  operation
}

///|
/// Start a callback-based asynchronous metadata lookup.
///
/// The callback is delivered from a later microtask or timer turn, never from
/// the initiating call stack.
pub fn AsyncOperator::stat_callback(
  self : AsyncOperator,
  path : StringView,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
  callback~ : (Result[Metadata, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let path = path.to_owned()
  let context = error_context(Stat, path~)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let handles : Array[UInt] = []
  defer release_bridge_inputs(handles)
  let path_handle = copy_bridge_input(path, handles, context)
  let version_handle = copy_optional_bridge_input(version, handles, context)
  let if_match_handle = copy_optional_bridge_input(if_match, handles, context)
  let if_none_match_handle = copy_optional_bridge_input(
    if_none_match, handles, context,
  )
  let task = bridge_operator_stat_options_start_v1(
    self.native.handle,
    path_handle,
    version_handle,
    if_match_handle,
    if_none_match_handle,
  )
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous stat",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_metadata_operation(
      operation,
      task,
      COMPLETION_STAT,
      "stat",
      context,
      callback,
    )
  })
  operation
}

///|
/// Start a callback-based native copy and return destination metadata.
///
/// The bridge requires the base backend's copy capability and never simulates
/// this operation with read plus write.
pub fn AsyncOperator::copy_callback(
  self : AsyncOperator,
  source : StringView,
  destination : StringView,
  callback~ : (Result[Metadata, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let source = source.to_owned()
  let destination = destination.to_owned()
  let context = error_context(Copy, path=source, destination_path=destination)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let handles : Array[UInt] = []
  defer release_bridge_inputs(handles)
  let source_handle = copy_bridge_input(source, handles, context)
  let destination_handle = copy_bridge_input(destination, handles, context)
  let task = bridge_operator_copy_start(
    self.native.handle,
    source_handle,
    destination_handle,
  )
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous copy",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_metadata_operation(
      operation,
      task,
      COMPLETION_COPY,
      "copy",
      context,
      callback,
    )
  })
  operation
}

///|
/// Start a callback-based native rename.
///
/// This operation is never simulated with copy plus delete.
pub fn AsyncOperator::rename_callback(
  self : AsyncOperator,
  source : StringView,
  destination : StringView,
  callback~ : (Result[Unit, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let source = source.to_owned()
  let destination = destination.to_owned()
  let context = error_context(Rename, path=source, destination_path=destination)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let handles : Array[UInt] = []
  defer release_bridge_inputs(handles)
  let source_handle = copy_bridge_input(source, handles, context)
  let destination_handle = copy_bridge_input(destination, handles, context)
  let task = bridge_operator_rename_start(
    self.native.handle,
    source_handle,
    destination_handle,
  )
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous rename",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_unit_operation(
      operation,
      task,
      COMPLETION_RENAME,
      "rename",
      context,
      callback,
    )
  })
  operation
}

///|
/// Start callback-based recursive directory creation.
///
/// The path must end with `/`. The callback is always delivered from a later
/// browser event-loop turn.
pub fn AsyncOperator::create_dir_callback(
  self : AsyncOperator,
  path : StringView,
  callback~ : (Result[Unit, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let path = path.to_owned()
  let context = error_context(CreateDir, path~)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path, context)
  let task = bridge_operator_create_dir_start(self.native.handle, path_handle)
  ignore(bridge_buffer_release(path_handle))
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start asynchronous directory creation",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_unit_operation(
      operation,
      task,
      COMPLETION_CREATE_DIR,
      "create_dir",
      context,
      callback,
    )
  })
  operation
}

///|
/// Start a callback-based delete. Deleting a missing path is successful.
///
/// The optional version and recursive behavior are passed through to the
/// selected OpenDAL backend. The callback is delivered from a later browser
/// event-loop turn.
pub fn AsyncOperator::delete_callback(
  self : AsyncOperator,
  path : StringView,
  version? : StringView,
  recursive? : Bool = false,
  callback~ : (Result[Unit, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let path = path.to_owned()
  let context = error_context(Delete, path~)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path, context)
  let version_handle = match version {
    None => 0U
    Some(version) =>
      copy_path_to_bridge(version, context) catch {
        error => {
          ignore(bridge_buffer_release(path_handle))
          raise error
        }
      }
  }
  let task = bridge_operator_delete_start(
    self.native.handle,
    path_handle,
    version_handle,
    recursive,
  )
  if version_handle != 0U {
    ignore(bridge_buffer_release(version_handle))
  }
  ignore(bridge_buffer_release(path_handle))
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous delete",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_unit_operation(
      operation,
      task,
      COMPLETION_DELETE,
      "delete",
      context,
      callback,
    )
  })
  operation
}

///|
fn entry_mode_from_bridge_unchecked(mode : Int) -> EntryMode {
  match mode {
    1 => File
    2 => Directory
    _ => Unknown
  }
}

///|
fn materialize_entry_list(
  handle : UInt,
  context : ErrorContext,
) -> Array[Entry] raise OpenDalError {
  let length = bridge_entry_list_len(handle)
  if length < 0 {
    raise_last_bridge_error(
      Unexpected,
      "could not query the entry-list length",
      context,
    )
  }
  let entries : Array[Entry] = []
  for index = 0; index < length; index = index + 1 {
    let index = index.reinterpret_as_uint()
    let path = take_utf8_buffer(
      bridge_entry_list_path(handle, index),
      "could not copy a listed path",
      context,
    )
    let name = take_utf8_buffer(
      bridge_entry_list_name(handle, index),
      "could not copy a listed name",
      context,
    )
    let metadata_buffer = bridge_entry_list_metadata_snapshot(handle, index)
    if metadata_buffer == 0U {
      raise_last_bridge_error(
        Unexpected,
        "could not take listed metadata",
        context,
      )
    }
    let snapshot = copy_buffer_from_bridge(metadata_buffer, context) catch {
      error => {
        ignore(bridge_buffer_release(metadata_buffer))
        raise error
      }
    }
    ignore(bridge_buffer_release(metadata_buffer))
    let metadata = parse_metadata_snapshot(snapshot, context)
    entries.push({ path, name, metadata })
  }
  entries
}

///|
/// Start a callback-based bounded prefix listing.
///
/// `limit` is a backend request/page hint. Independently, the bridge enforces
/// fixed total entry and UTF-8 byte ceilings before publishing any result.
pub fn AsyncOperator::list_callback(
  self : AsyncOperator,
  path : StringView,
  recursive? : Bool = false,
  limit? : UInt64,
  start_after? : StringView,
  callback~ : (Result[Array[Entry], OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let path = path.to_owned()
  let context = error_context(List, path~)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path, context)
  let start_after_handle = match start_after {
    None => 0U
    Some(start_after) =>
      copy_path_to_bridge(start_after, context) catch {
        error => {
          ignore(bridge_buffer_release(path_handle))
          raise error
        }
      }
  }
  let task = bridge_operator_list_start(
    self.native.handle,
    path_handle,
    recursive,
    limit is Some(_),
    limit.unwrap_or(0UL),
    start_after_handle,
  )
  if start_after_handle != 0U {
    ignore(bridge_buffer_release(start_after_handle))
  }
  ignore(bridge_buffer_release(path_handle))
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous list",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    match operation.take_completion(task) {
      CompletionLocalMismatch =>
        callback(
          Err(
            local_error(
              ResourceClosed,
              "list task no longer owns the pending callback",
              context,
            ),
          ),
        )
      CompletionBridgeFailure =>
        callback(
          Err(
            take_bridge_error(
              ResourceClosed,
              "list task became ready without a completion",
              context,
            ),
          ),
        )
      CompletionTaken(completion) => {
        let kind = bridge_completion_kind(completion)
        let status = bridge_completion_status(completion)
        if kind != COMPLETION_LIST {
          ignore(bridge_completion_release(completion))
          callback(
            Err(
              local_error(
                AbiMismatch,
                "list task returned the wrong completion kind",
                context,
              ),
            ),
          )
        } else if status != 0U {
          callback(Err(completion_error(completion, "list failed", context)))
        } else {
          let entry_list = bridge_completion_take_entry_list(completion)
          if entry_list == 0U {
            ignore(bridge_completion_release(completion))
            callback(
              Err(
                take_bridge_error(
                  Unexpected,
                  "could not take the list result",
                  context,
                ),
              ),
            )
          } else {
            let result = try materialize_entry_list(entry_list, context) catch {
              error => Err(error)
            } noraise {
              entries => Ok(entries)
            }
            ignore(bridge_entry_list_release(entry_list))
            callback(result)
          }
        }
      }
    }
  })
  operation
}
