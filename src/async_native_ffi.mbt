///|
/// A native asynchronous operation. The C external-object finalizer cancels
/// unfinished work and releases the task. The worker owns no MoonBit value.
priv type NativeAsyncTask

///|
/// A native sequential cursor used only by `AsyncReadStream`.
priv type NativeAsyncReadStream

///|
/// A native chunked writer used only by `AsyncWriter`.
priv type NativeAsyncWriter

///|
/// Start one whole-object read. Every string and byte view is copied before
/// this call returns, and `completion_fd` is duplicated before the caller
/// closes its pipe endpoint.
#borrow(operator, path, version, if_match, if_none_match)
extern "C" fn native_async_operator_read_start(
  operator : Operator,
  path : String,
  range_kind : UInt,
  range_offset : UInt64,
  range_length : UInt64,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
  completion_fd : Int,
) -> NativeResult = "moonbit_opendal_async_operator_read_start"

///|
/// Start opening one bounded sequential read stream.
#borrow(operator, path, version, if_match, if_none_match)
extern "C" fn native_async_operator_read_stream_start(
  operator : Operator,
  path : String,
  range_kind : UInt,
  range_offset : UInt64,
  range_length : UInt64,
  chunk_size : Int,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
  completion_fd : Int,
) -> NativeResult = "moonbit_opendal_async_operator_read_stream_start"

///|
/// Start fetching the next chunk. The native handle admits one in-flight
/// operation; cancelling it leaves the stream in its terminal Failed state.
#borrow(stream)
extern "C" fn native_async_read_stream_next_start(
  stream : NativeAsyncReadStream,
  completion_fd : Int,
) -> NativeResult = "moonbit_opendal_async_read_stream_next_start"

///|
/// Close a stream synchronously. Closing more than once is harmless and also
/// cancels any in-flight operation without starting background work.
#borrow(stream)
extern "C" fn native_async_read_stream_close(
  stream : NativeAsyncReadStream,
) -> Unit = "moonbit_opendal_async_read_stream_close"

///|
/// Start opening one asynchronous chunked writer.
#borrow(operator, path, content_type, content_disposition, content_encoding, cache_control, if_match, if_none_match)
extern "C" fn native_async_operator_writer_start(
  operator : Operator,
  path : String,
  append : Bool,
  has_content_type : Bool,
  content_type : String,
  has_content_disposition : Bool,
  content_disposition : String,
  has_content_encoding : Bool,
  content_encoding : String,
  has_cache_control : Bool,
  cache_control : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
  completion_fd : Int,
) -> NativeResult = "moonbit_opendal_async_operator_writer_start"

///|
/// Start writing one owned copy of a chunk. Cancelling the task leaves the
/// writer in its terminal Failed state because commit status can be unknown.
#borrow(writer, data)
extern "C" fn native_async_writer_write_start(
  writer : NativeAsyncWriter,
  data : Bytes,
  completion_fd : Int,
) -> NativeResult = "moonbit_opendal_async_writer_write_start"

///|
/// Start committing a writer. The first finish attempt is terminal.
#borrow(writer)
extern "C" fn native_async_writer_finish_start(
  writer : NativeAsyncWriter,
  completion_fd : Int,
) -> NativeResult = "moonbit_opendal_async_writer_finish_start"

///|
/// Start aborting a writer. Successful abort remains idempotent.
#borrow(writer)
extern "C" fn native_async_writer_abort_start(
  writer : NativeAsyncWriter,
  completion_fd : Int,
) -> NativeResult = "moonbit_opendal_async_writer_abort_start"

///|
/// Take the task created by a successful start result.
#borrow(result)
extern "C" fn native_result_take_async_task(
  result : NativeResult,
) -> NativeAsyncTask = "moonbit_opendal_result_take_async_task"

///|
/// Take the stream created by a successful open result.
#borrow(result)
extern "C" fn native_result_take_async_read_stream(
  result : NativeResult,
) -> NativeAsyncReadStream = "moonbit_opendal_result_take_async_read_stream"

///|
/// Take the writer created by a successful open result.
#borrow(result)
extern "C" fn native_result_take_async_writer(
  result : NativeResult,
) -> NativeAsyncWriter = "moonbit_opendal_result_take_async_writer"

///|
/// Request cancellation. This operation is synchronous and idempotent. A
/// task finalizer performs the same operation when MoonBit drops the handle.
/// For a stream or writer operation, cancelling before its result is taken
/// makes that resource Failed, including the completed-but-unclaimed race.
/// Cancelling after a successful take is a no-op.
#borrow(task)
extern "C" fn native_async_task_cancel(task : NativeAsyncTask) -> Unit = "moonbit_opendal_async_task_cancel"

///|
/// Take a completed task's owned result exactly once. Calling this before the
/// completion byte or calling it again produces a native ResourceClosed error.
#borrow(task)
extern "C" fn native_async_task_take_result(
  task : NativeAsyncTask,
) -> NativeResult = "moonbit_opendal_async_task_take_result"
