///|
const TASK_READY = 2U

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
/// Return the local state of this experimental callback operation.
pub fn Operation::state(self : Operation) -> OperationState {
  self.state
}

///|
/// Request logical cancellation. Cancelling more than once is harmless.
///
/// Cancellation suppresses the user callback. The underlying browser I/O may
/// continue, but any late completion is discarded by the bridge.
pub fn Operation::cancel(self : Operation) -> Unit {
  if self.state is Pending && self.handle != 0U {
    ignore(bridge_task_cancel(self.handle))
    ignore(bridge_task_release(self.handle))
    self.handle = 0U
    self.state = Cancelled
  }
}

///|
/// Release this operation without delivering a later callback.
///
/// Closing more than once is harmless.
pub fn Operation::close(self : Operation) -> Unit {
  if self.state is Pending {
    self.cancel()
    self.state = Closed
  } else if self.state is Cancelled {
    self.state = Closed
  }
}

///|
fn Operation::take_completion(self : Operation, task : UInt) -> UInt? {
  if self.state != Pending || self.handle != task {
    return None
  }
  let completion = bridge_task_take(task)
  ignore(bridge_task_release(task))
  self.handle = 0U
  self.state = Completed
  if completion == 0U {
    None
  } else {
    Some(completion)
  }
}

///|
fn completion_error(
  completion : UInt,
  status : UInt,
  fallback : String,
) -> WasmError {
  let error = bridge_completion_take_error(completion)
  if error == 0U {
    ignore(bridge_completion_release(completion))
    return take_bridge_error(status.reinterpret_as_int(), fallback)
  }
  take_owned_bridge_error(error, status.reinterpret_as_int(), fallback)
}

///|
fn start_wait(operation : Operation, finish : (UInt) -> Unit) -> Unit {
  host_wait_task(operation.handle, task => {
    if bridge_task_state(task) == TASK_READY {
      finish(task)
    }
  })
}

///|
fn finish_unit_operation(
  operation : Operation,
  task : UInt,
  expected_kind : UInt,
  name : String,
  callback : (Result[Unit, WasmError]) -> Unit,
) -> Unit {
  match operation.take_completion(task) {
    None =>
      callback(
        Err(
          take_bridge_error(
            12,
            "\{name} task became ready without a completion",
          ),
        ),
      )
    Some(completion) => {
      let kind = bridge_completion_kind(completion)
      let status = bridge_completion_status(completion)
      if kind != expected_kind {
        ignore(bridge_completion_release(completion))
        callback(
          Err(WasmError(2, "\{name} task returned the wrong completion kind")),
        )
      } else if status != 0U {
        callback(Err(completion_error(completion, status, "\{name} failed")))
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
/// The callback is always delivered from a later browser event-loop turn.
pub fn Operator::write_callback(
  self : Operator,
  path : StringView,
  data : BytesView,
  callback : (Result[Unit, WasmError]) -> Unit,
) -> Operation raise WasmError {
  if self.handle == 0U {
    raise WasmError(1, "operator is closed")
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path)
  let data_handle = copy_bytes_to_bridge(data) catch {
    error => {
      ignore(bridge_buffer_release(path_handle))
      raise error
    }
  }
  let task = bridge_operator_write_start(self.handle, path_handle, data_handle)
  ignore(bridge_buffer_release(data_handle))
  ignore(bridge_buffer_release(path_handle))
  if task == 0U {
    raise_last_bridge_error("could not start an asynchronous write")
  }
  let operation : Operation = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_unit_operation(operation, task, COMPLETION_WRITE, "write", callback)
  })
  operation
}

///|
/// Start a callback-based asynchronous whole-object read.
///
/// The callback is always delivered from a later browser event-loop turn.
pub fn Operator::read_callback(
  self : Operator,
  path : StringView,
  callback : (Result[Bytes, WasmError]) -> Unit,
) -> Operation raise WasmError {
  if self.handle == 0U {
    raise WasmError(1, "operator is closed")
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path)
  let task = bridge_operator_read_start(self.handle, path_handle)
  ignore(bridge_buffer_release(path_handle))
  if task == 0U {
    raise_last_bridge_error("could not start an asynchronous read")
  }
  let operation : Operation = { handle: task, state: Pending }
  start_wait(operation, task => {
    match operation.take_completion(task) {
      None =>
        callback(
          Err(
            take_bridge_error(12, "read task became ready without a completion"),
          ),
        )
      Some(completion) => {
        let kind = bridge_completion_kind(completion)
        let status = bridge_completion_status(completion)
        if kind != COMPLETION_READ {
          ignore(bridge_completion_release(completion))
          callback(
            Err(WasmError(2, "read task returned the wrong completion kind")),
          )
        } else if status != 0U {
          callback(Err(completion_error(completion, status, "read failed")))
        } else {
          let buffer = bridge_completion_take_buffer(completion)
          if buffer == 0U {
            ignore(bridge_completion_release(completion))
            callback(
              Err(take_bridge_error(11, "could not take the read result")),
            )
          } else {
            let result = try copy_buffer_from_bridge(buffer) catch {
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
/// The callback is always delivered from a later browser event-loop turn.
pub fn Operator::stat_callback(
  self : Operator,
  path : StringView,
  callback : (Result[Metadata, WasmError]) -> Unit,
) -> Operation raise WasmError {
  if self.handle == 0U {
    raise WasmError(1, "operator is closed")
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path)
  let task = bridge_operator_stat_start(self.handle, path_handle)
  ignore(bridge_buffer_release(path_handle))
  if task == 0U {
    raise_last_bridge_error("could not start an asynchronous stat")
  }
  let operation : Operation = { handle: task, state: Pending }
  start_wait(operation, task => {
    match operation.take_completion(task) {
      None =>
        callback(
          Err(
            take_bridge_error(12, "stat task became ready without a completion"),
          ),
        )
      Some(completion) => {
        let kind = bridge_completion_kind(completion)
        let status = bridge_completion_status(completion)
        if kind != COMPLETION_STAT {
          ignore(bridge_completion_release(completion))
          callback(
            Err(WasmError(2, "stat task returned the wrong completion kind")),
          )
        } else if status != 0U {
          callback(Err(completion_error(completion, status, "stat failed")))
        } else {
          let metadata = bridge_completion_take_metadata(completion)
          if metadata == 0U {
            ignore(bridge_completion_release(completion))
            callback(
              Err(take_bridge_error(11, "could not take the stat result")),
            )
          } else {
            ignore(bridge_last_error_clear())
            let is_file = bridge_metadata_is_file(metadata)
            let low = bridge_metadata_content_length_low(metadata)
            let high = bridge_metadata_content_length_high(metadata)
            let metadata_error = bridge_last_error_code()
            ignore(bridge_metadata_release(metadata))
            if is_file < 0 || metadata_error != 0U {
              callback(Err(take_bridge_error(-1, "could not inspect metadata")))
            } else {
              callback(
                Ok({
                  content_length: combine_u64(low, high),
                  is_file: is_file != 0,
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
pub fn Operator::create_dir_callback(
  self : Operator,
  path : StringView,
  callback : (Result[Unit, WasmError]) -> Unit,
) -> Operation raise WasmError {
  if self.handle == 0U {
    raise WasmError(1, "operator is closed")
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path)
  let task = bridge_operator_create_dir_start(self.handle, path_handle)
  ignore(bridge_buffer_release(path_handle))
  if task == 0U {
    raise_last_bridge_error("could not start asynchronous directory creation")
  }
  let operation : Operation = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_unit_operation(
      operation,
      task,
      COMPLETION_CREATE_DIR,
      "create_dir",
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
pub fn Operator::delete_callback(
  self : Operator,
  path : StringView,
  callback : (Result[Unit, WasmError]) -> Unit,
  version? : StringView,
  recursive? : Bool = false,
) -> Operation raise WasmError {
  if self.handle == 0U {
    raise WasmError(1, "operator is closed")
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path)
  let version_handle = match version {
    None => 0U
    Some(version) =>
      copy_path_to_bridge(version) catch {
        error => {
          ignore(bridge_buffer_release(path_handle))
          raise error
        }
      }
  }
  let task = bridge_operator_delete_start(
    self.handle,
    path_handle,
    version_handle,
    recursive,
  )
  if version_handle != 0U {
    ignore(bridge_buffer_release(version_handle))
  }
  ignore(bridge_buffer_release(path_handle))
  if task == 0U {
    raise_last_bridge_error("could not start an asynchronous delete")
  }
  let operation : Operation = { handle: task, state: Pending }
  start_wait(operation, task => {
    finish_unit_operation(
      operation,
      task,
      COMPLETION_DELETE,
      "delete",
      callback,
    )
  })
  operation
}
