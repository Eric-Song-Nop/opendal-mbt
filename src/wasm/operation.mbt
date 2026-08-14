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
/// Start a callback-based asynchronous write.
///
/// The callback is delivered from a later microtask or timer turn, never from
/// the initiating call stack.
pub fn AsyncOperator::write_callback(
  self : AsyncOperator,
  path : StringView,
  data : BytesView,
  callback~ : (Result[Unit, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let path = path.to_owned()
  let context = error_context(Write, path~)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path, context)
  let data_handle = copy_bytes_to_bridge(data, context) catch {
    error => {
      ignore(bridge_buffer_release(path_handle))
      raise error
    }
  }
  let task = bridge_operator_write_start(
    self.native.handle,
    path_handle,
    data_handle,
  )
  ignore(bridge_buffer_release(data_handle))
  ignore(bridge_buffer_release(path_handle))
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous write",
      context,
    )
  }
  let operation : Task = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_unit_operation(
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
/// Start a callback-based asynchronous whole-object read.
///
/// The callback is delivered from a later microtask or timer turn, never from
/// the initiating call stack.
pub fn AsyncOperator::read_callback(
  self : AsyncOperator,
  path : StringView,
  callback~ : (Result[Bytes, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let path = path.to_owned()
  let context = error_context(Read, path~)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path, context)
  let task = bridge_operator_read_start(self.native.handle, path_handle)
  ignore(bridge_buffer_release(path_handle))
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
  callback~ : (Result[Metadata, OpenDalError]) -> Unit,
) -> Task raise OpenDalError {
  let path = path.to_owned()
  let context = error_context(Stat, path~)
  if self.native.handle == 0U {
    raise local_error(ResourceClosed, "operator is closed", context)
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path, context)
  let task = bridge_operator_stat_start(self.native.handle, path_handle)
  ignore(bridge_buffer_release(path_handle))
  if task == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not start an asynchronous stat",
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
              "stat task no longer owns the pending callback",
              context,
            ),
          ),
        )
      CompletionBridgeFailure =>
        callback(
          Err(
            take_bridge_error(
              ResourceClosed,
              "stat task became ready without a completion",
              context,
            ),
          ),
        )
      CompletionTaken(completion) => {
        let kind = bridge_completion_kind(completion)
        let status = bridge_completion_status(completion)
        if kind != COMPLETION_STAT {
          ignore(bridge_completion_release(completion))
          callback(
            Err(
              local_error(
                AbiMismatch,
                "stat task returned the wrong completion kind",
                context,
              ),
            ),
          )
        } else if status != 0U {
          callback(Err(completion_error(completion, "stat failed", context)))
        } else {
          let metadata = bridge_completion_take_metadata(completion)
          if metadata == 0U {
            ignore(bridge_completion_release(completion))
            callback(
              Err(
                take_bridge_error(
                  Unexpected,
                  "could not take the stat result",
                  context,
                ),
              ),
            )
          } else {
            ignore(bridge_last_error_clear())
            let mode = bridge_metadata_mode(metadata)
            let low = bridge_metadata_content_length_low(metadata)
            let high = bridge_metadata_content_length_high(metadata)
            let metadata_error = bridge_last_error_take()
            ignore(bridge_metadata_release(metadata))
            if metadata_error != 0U {
              callback(
                Err(
                  take_owned_bridge_error(
                    metadata_error,
                    Unexpected,
                    "could not inspect metadata",
                    context,
                  ),
                ),
              )
            } else if mode < 0 || mode > 2 {
              callback(
                Err(
                  local_error(
                    AbiMismatch,
                    "bridge returned an invalid metadata mode",
                    context,
                  ),
                ),
              )
            } else {
              callback(
                Ok({
                  mode: entry_mode_from_bridge_unchecked(mode),
                  content_length: combine_u64(low, high),
                }),
              )
            }
          }
        }
      }
    }
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
  callback~ : (Result[Unit, OpenDalError]) -> Unit,
  version? : StringView,
  recursive? : Bool = false,
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
fn entry_mode_from_bridge(
  mode : Int,
  context : ErrorContext,
) -> EntryMode raise OpenDalError {
  if mode < 0 || mode > 2 {
    raise local_error(
      AbiMismatch,
      "bridge returned an invalid entry mode \{mode}",
      context,
    )
  }
  entry_mode_from_bridge_unchecked(mode)
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
    ignore(bridge_last_error_clear())
    let mode = bridge_entry_list_mode(handle, index)
    let content_length = bridge_entry_list_content_length(handle, index)
    let entry_error = bridge_last_error_take()
    if entry_error != 0U {
      raise take_owned_bridge_error(
        entry_error,
        Unexpected,
        "could not inspect a listed entry",
        context,
      )
    }
    if mode < 0 || mode > 2 {
      raise local_error(
        AbiMismatch,
        "bridge returned an invalid listed entry mode",
        context,
      )
    }
    entries.push({
      path,
      name,
      metadata: { mode: entry_mode_from_bridge(mode, context), content_length },
    })
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
  callback~ : (Result[Array[Entry], OpenDalError]) -> Unit,
  recursive? : Bool = false,
  limit? : UInt64,
  start_after? : StringView,
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
