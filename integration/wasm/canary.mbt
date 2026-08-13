///|
const CANARY_OK = 0

///|
const CANARY_UNEXPECTED_ERROR = 1

///|
const CANARY_ROUNDTRIP_MISMATCH = 2

///|
const CANARY_METADATA_MISMATCH = 3

///|
const CANARY_MISSING_PATH_WAS_READ = 4

///|
const CANARY_WRONG_MISSING_ERROR = 5

///|
const CANARY_HANDLE_LEAK = 6

///|
const CANARY_SERVICE_INFO_MISMATCH = 7

///|
const CANARY_BULK_TRANSFER_MISMATCH = 8

///|
const BULK_TRANSFER_BYTES = 16777216

///|
const ASYNC_IDLE = 0

///|
const ASYNC_CREATE_DIR_PENDING = 1

///|
const ASYNC_WRITE_PENDING = 2

///|
const ASYNC_READ_PENDING = 3

///|
const ASYNC_STAT_PENDING = 4

///|
const ASYNC_LIST_PENDING = 5

///|
const ASYNC_DELETE_PENDING = 6

///|
const ASYNC_DELETE_AGAIN_PENDING = 7

///|
const ASYNC_MISSING_PENDING = 8

///|
const ASYNC_OK = 9

///|
const ASYNC_ERROR = -1

///|
const RACE_IDLE = 0

///|
const RACE_PENDING = 1

///|
const RACE_COMPLETED = 2

///|
const RACE_CANCELLED = 3

///|
const RACE_CLOSED = 4

///|
const RACE_ERROR = -1

///|
let async_stage : Ref[Int] = Ref(ASYNC_IDLE)

///|
let async_baseline : Ref[UInt] = Ref(0U)

///|
let async_operator : Ref[@opendal.Operator?] = Ref(None)

///|
let async_operation : Ref[@opendal.Operation?] = Ref(None)

///|
let cancel_callback_count : Ref[Int] = Ref(0)

///|
let cancel_baseline : Ref[UInt] = Ref(0U)

///|
let cancel_operator : Ref[@opendal.Operator?] = Ref(None)

///|
let cancel_operation : Ref[@opendal.Operation?] = Ref(None)

///|
let ready_baseline : Ref[UInt] = Ref(0U)

///|
let ready_callback_count : Ref[Int] = Ref(0)

///|
let ready_callback_ok : Ref[Bool] = Ref(false)

///|
let ready_operator : Ref[@opendal.Operator?] = Ref(None)

///|
let ready_operation : Ref[@opendal.Operation?] = Ref(None)

///|
let multi_baseline : Ref[UInt] = Ref(0U)

///|
let multi_stage : Ref[Int] = Ref(RACE_IDLE)

///|
let multi_mask : Ref[Int] = Ref(0)

///|
let multi_callback_count : Ref[Int] = Ref(0)

///|
let multi_operator_one : Ref[@opendal.Operator?] = Ref(None)

///|
let multi_operator_two : Ref[@opendal.Operator?] = Ref(None)

///|
let multi_operation_one : Ref[@opendal.Operation?] = Ref(None)

///|
let multi_operation_two : Ref[@opendal.Operation?] = Ref(None)

///|
let dispose_callback_count : Ref[Int] = Ref(0)

///|
let dispose_operator : Ref[@opendal.Operator?] = Ref(None)

///|
let dispose_operation : Ref[@opendal.Operation?] = Ref(None)

///|
fn fail_async_lifecycle() -> Unit {
  if async_operation.val is Some(operation) {
    operation.close()
  }
  if async_operator.val is Some(operator) {
    operator.close()
  }
  async_operation.val = None
  async_operator.val = None
  async_stage.val = ASYNC_ERROR
}

///|
fn start_async_missing(operator : @opendal.Operator) -> Unit {
  async_stage.val = ASYNC_MISSING_PENDING
  let operation = operator.read_callback("canary/async/data.bin", result => {
    match result {
      Err(error) if error.is_not_found() => {
        async_operation.val = None
        async_operator.val = None
        if @opendal.live_handle_count() == async_baseline.val {
          async_stage.val = ASYNC_OK
        } else {
          async_stage.val = ASYNC_ERROR
        }
      }
      _ => fail_async_lifecycle()
    }
  }) catch {
    _ => {
      fail_async_lifecycle()
      return
    }
  }
  async_operation.val = Some(operation)
  operator.close()
}

///|
fn start_async_delete_again(operator : @opendal.Operator) -> Unit {
  async_stage.val = ASYNC_DELETE_AGAIN_PENDING
  let operation = operator.delete_callback(
    "canary/async/",
    result => {
      match result {
        Ok(_) => start_async_missing(operator)
        Err(_) => fail_async_lifecycle()
      }
    },
    recursive=true,
  ) catch {
    _ => {
      fail_async_lifecycle()
      return
    }
  }
  async_operation.val = Some(operation)
}

///|
fn start_async_delete(operator : @opendal.Operator) -> Unit {
  async_stage.val = ASYNC_DELETE_PENDING
  let operation = operator.delete_callback(
    "canary/async/",
    result => {
      match result {
        Ok(_) => start_async_delete_again(operator)
        Err(_) => fail_async_lifecycle()
      }
    },
    recursive=true,
  ) catch {
    _ => {
      fail_async_lifecycle()
      return
    }
  }
  async_operation.val = Some(operation)
}

///|
fn start_async_list(operator : @opendal.Operator, payload : Bytes) -> Unit {
  async_stage.val = ASYNC_LIST_PENDING
  let operation = operator.list_callback(
    "canary/async/",
    result => {
      match result {
        Ok(entries) => {
          let mut found_file = false
          let mut found_directory = false
          for entry in entries {
            if entry.path == "canary/async/data.bin" &&
              entry.name == "data.bin" &&
              entry.is_file() &&
              entry.metadata.content_length == payload.length().to_uint64() {
              found_file = true
            } else if entry.path == "canary/async/" && entry.is_dir() {
              found_directory = true
            }
          }
          if found_file && found_directory {
            start_async_delete(operator)
          } else {
            fail_async_lifecycle()
          }
        }
        Err(_) => fail_async_lifecycle()
      }
    },
    recursive=true,
    limit=2UL,
  ) catch {
    _ => {
      fail_async_lifecycle()
      return
    }
  }
  async_operation.val = Some(operation)
}

///|
fn start_async_stat(operator : @opendal.Operator, payload : Bytes) -> Unit {
  async_stage.val = ASYNC_STAT_PENDING
  let operation = operator.stat_callback("canary/async/data.bin", result => {
    match result {
      Ok(metadata) if metadata.is_file() &&
        metadata.content_length == payload.length().to_uint64() =>
        start_async_list(operator, payload)
      _ => fail_async_lifecycle()
    }
  }) catch {
    _ => {
      fail_async_lifecycle()
      return
    }
  }
  async_operation.val = Some(operation)
}

///|
fn start_async_read(operator : @opendal.Operator, payload : Bytes) -> Unit {
  async_stage.val = ASYNC_READ_PENDING
  let operation = operator.read_callback("canary/async/data.bin", result => {
    match result {
      Ok(actual) if actual == payload => start_async_stat(operator, payload)
      _ => fail_async_lifecycle()
    }
  }) catch {
    _ => {
      fail_async_lifecycle()
      return
    }
  }
  async_operation.val = Some(operation)
}

///|
fn start_async_write(operator : @opendal.Operator, payload : Bytes) -> Unit {
  async_stage.val = ASYNC_WRITE_PENDING
  let operation = operator.write_callback("canary/async/data.bin", payload, result => {
    match result {
      Ok(_) => start_async_read(operator, payload)
      Err(_) => fail_async_lifecycle()
    }
  }) catch {
    _ => {
      fail_async_lifecycle()
      return
    }
  }
  async_operation.val = Some(operation)
}

///|
/// Start a forced-pending asynchronous memory round trip.
#export_name("opendal_mbt_wasm_canary_async_start")
pub fn canary_async_start() -> Int {
  if async_stage.val != ASYNC_IDLE && async_stage.val != ASYNC_OK {
    return ASYNC_ERROR
  }
  async_baseline.val = @opendal.live_handle_count()
  let operator = @opendal.Operator::new("memory") catch {
    _ => return ASYNC_ERROR
  }
  async_operator.val = Some(operator)
  let payload = b"\x00\xffMoonBit-async\x80OpenDAL"
  async_stage.val = ASYNC_CREATE_DIR_PENDING
  let operation = operator.create_dir_callback("canary/async/", result => {
    match result {
      Ok(_) => start_async_write(operator, payload)
      Err(_) => fail_async_lifecycle()
    }
  }) catch {
    _ => {
      fail_async_lifecycle()
      return ASYNC_ERROR
    }
  }
  async_operation.val = Some(operation)
  ASYNC_CREATE_DIR_PENDING
}

///|
/// Return the current asynchronous lifecycle stage.
#export_name("opendal_mbt_wasm_canary_async_stage")
pub fn canary_async_stage() -> Int {
  async_stage.val
}

///|
/// Return the forced-pending poll count through the MoonBit facade.
#export_name("opendal_mbt_wasm_canary_pending_poll_count")
pub fn canary_pending_poll_count() -> UInt {
  @opendal.canary_forced_pending_poll_count()
}

///|
/// Cancel and release a task before its forced delay completes.
#export_name("opendal_mbt_wasm_canary_cancel_start")
pub fn canary_cancel_start() -> Int {
  cancel_callback_count.val = 0
  cancel_baseline.val = @opendal.live_handle_count()
  let operator = @opendal.Operator::new("memory") catch {
    _ => return ASYNC_ERROR
  }
  cancel_operator.val = Some(operator)
  let operation = operator.read_callback("canary/cancel.bin", _ => {
    cancel_callback_count.val = cancel_callback_count.val + 1
  }) catch {
    _ => {
      operator.close()
      cancel_operator.val = None
      return ASYNC_ERROR
    }
  }
  cancel_operation.val = Some(operation)
  operation.cancel()
  operator.close()
  cancel_operation.val = None
  cancel_operator.val = None
  0
}

///|
/// Return the number of user callbacks delivered after cancellation.
#export_name("opendal_mbt_wasm_canary_cancel_callback_count")
pub fn canary_cancel_callback_count() -> Int {
  cancel_callback_count.val
}

///|
/// Return whether cancellation restored the live-handle baseline.
#export_name("opendal_mbt_wasm_canary_cancel_is_clean")
pub fn canary_cancel_is_clean() -> Int {
  if @opendal.live_handle_count() == cancel_baseline.val {
    1
  } else {
    0
  }
}

///|
fn operation_state_code(operation : @opendal.Operation) -> Int {
  match operation.state() {
    @opendal.Pending => RACE_PENDING
    @opendal.Completed => RACE_COMPLETED
    @opendal.Cancelled => RACE_CANCELLED
    @opendal.Closed => RACE_CLOSED
  }
}

///|
fn ready_status_value() -> Int {
  if ready_callback_count.val > 1 ||
    (ready_callback_count.val == 1 && !ready_callback_ok.val) {
    return RACE_ERROR
  }
  match ready_operation.val {
    None => RACE_IDLE
    Some(operation) => {
      let state = operation_state_code(operation)
      if (state == RACE_COMPLETED || state == RACE_CLOSED) &&
        ready_callback_count.val != 1 {
        RACE_ERROR
      } else {
        state
      }
    }
  }
}

///|
/// Start a task that will complete with a stable `NotFound` result.
#export_name("opendal_mbt_wasm_canary_ready_start")
pub fn canary_ready_start() -> Int {
  if ready_status_value() == RACE_PENDING {
    return RACE_ERROR
  }
  ready_baseline.val = @opendal.live_handle_count()
  ready_callback_count.val = 0
  ready_callback_ok.val = false
  let operator = @opendal.Operator::new("memory") catch {
    _ => return RACE_ERROR
  }
  ready_operator.val = Some(operator)
  let operation = operator.read_callback("canary/ready-missing.bin", result => {
    ready_callback_count.val = ready_callback_count.val + 1
    ready_callback_ok.val = match result {
      Err(error) => error.is_not_found()
      Ok(_) => false
    }
  }) catch {
    _ => {
      operator.close()
      ready_operator.val = None
      return RACE_ERROR
    }
  }
  ready_operation.val = Some(operation)
  RACE_PENDING
}

///|
/// Return the completion-wins operation state and validate callback count.
#export_name("opendal_mbt_wasm_canary_ready_status")
pub fn canary_ready_status() -> Int {
  ready_status_value()
}

///|
/// Cancel after completion; this must not roll the state back from Completed.
#export_name("opendal_mbt_wasm_canary_ready_cancel")
pub fn canary_ready_cancel() -> Int {
  if ready_operation.val is Some(operation) {
    operation.cancel()
  }
  ready_status_value()
}

///|
/// Close a completed operation and release its retained operator.
#export_name("opendal_mbt_wasm_canary_ready_close")
pub fn canary_ready_close() -> Int {
  if ready_operation.val is Some(operation) {
    operation.close()
  }
  if ready_operator.val is Some(operator) {
    operator.close()
  }
  ready_operator.val = None
  let state = ready_status_value()
  if state == RACE_CLOSED && @opendal.live_handle_count() == ready_baseline.val {
    state
  } else {
    RACE_ERROR
  }
}

///|
fn close_multi_resources() -> Unit {
  if multi_operation_one.val is Some(operation) {
    operation.close()
  }
  if multi_operation_two.val is Some(operation) {
    operation.close()
  }
  if multi_operator_one.val is Some(operator) {
    operator.close()
  }
  if multi_operator_two.val is Some(operator) {
    operator.close()
  }
  multi_operation_one.val = None
  multi_operation_two.val = None
  multi_operator_one.val = None
  multi_operator_two.val = None
}

///|
fn fail_multi() -> Unit {
  close_multi_resources()
  multi_stage.val = RACE_ERROR
}

///|
fn finish_multi(
  bit : Int,
  expected : Bytes,
  result : Result[Bytes, @opendal.WasmError],
) -> Unit {
  multi_callback_count.val = multi_callback_count.val + 1
  if multi_callback_count.val > 2 {
    fail_multi()
    return
  }
  match result {
    Ok(actual) if actual == expected => {
      multi_mask.val = multi_mask.val | bit
      if multi_mask.val == 3 {
        close_multi_resources()
        if multi_callback_count.val == 2 &&
          @opendal.live_handle_count() == multi_baseline.val {
          multi_stage.val = RACE_COMPLETED
        } else {
          multi_stage.val = RACE_ERROR
        }
      }
    }
    _ => fail_multi()
  }
}

///|
/// Start reads against two independent operators at the same time.
#export_name("opendal_mbt_wasm_canary_multi_start")
pub fn canary_multi_start() -> Int {
  if multi_stage.val == RACE_PENDING {
    return RACE_ERROR
  }
  multi_baseline.val = @opendal.live_handle_count()
  multi_stage.val = RACE_PENDING
  multi_mask.val = 0
  multi_callback_count.val = 0
  let operator_one = @opendal.Operator::new("memory") catch {
    _ => {
      multi_stage.val = RACE_ERROR
      return RACE_ERROR
    }
  }
  multi_operator_one.val = Some(operator_one)
  let operator_two = @opendal.Operator::new("memory") catch {
    _ => {
      fail_multi()
      return RACE_ERROR
    }
  }
  multi_operator_two.val = Some(operator_two)
  let payload_one = b"operator-one\x00\xff"
  let payload_two = b"operator-two\x80\x01"
  operator_one.write("canary/shared.bin", payload_one) catch {
    _ => {
      fail_multi()
      return RACE_ERROR
    }
  }
  operator_two.write("canary/shared.bin", payload_two) catch {
    _ => {
      fail_multi()
      return RACE_ERROR
    }
  }
  let operation_one = operator_one.read_callback("canary/shared.bin", result => {
    finish_multi(1, payload_one, result)
  }) catch {
    _ => {
      fail_multi()
      return RACE_ERROR
    }
  }
  multi_operation_one.val = Some(operation_one)
  let operation_two = operator_two.read_callback("canary/shared.bin", result => {
    finish_multi(2, payload_two, result)
  }) catch {
    _ => {
      fail_multi()
      return RACE_ERROR
    }
  }
  multi_operation_two.val = Some(operation_two)
  RACE_PENDING
}

///|
/// Return the multi-operator lifecycle stage.
#export_name("opendal_mbt_wasm_canary_multi_stage")
pub fn canary_multi_stage() -> Int {
  multi_stage.val
}

///|
/// Start one pending operation that will be destroyed with the runtime.
#export_name("opendal_mbt_wasm_canary_dispose_start")
pub fn canary_dispose_start() -> Int {
  dispose_callback_count.val = 0
  let operator = @opendal.Operator::new("memory") catch {
    _ => return RACE_ERROR
  }
  dispose_operator.val = Some(operator)
  let operation = operator.read_callback("canary/dispose-missing.bin", _ => {
    dispose_callback_count.val = dispose_callback_count.val + 1
  }) catch {
    _ => {
      operator.close()
      dispose_operator.val = None
      return RACE_ERROR
    }
  }
  dispose_operation.val = Some(operation)
  RACE_PENDING
}

///|
/// Return the callback count without touching the torn-down bridge.
#export_name("opendal_mbt_wasm_canary_dispose_callback_count")
pub fn canary_dispose_callback_count() -> Int {
  dispose_callback_count.val
}

///|
fn run_one_lifecycle() -> Int {
  let schemes = @opendal.available_schemes() catch {
    _ => return CANARY_UNEXPECTED_ERROR
  }
  if schemes != ["memory"] {
    return CANARY_SERVICE_INFO_MISMATCH
  }
  let operator = @opendal.Operator::new("memory", config={
    "root": "generic-prefix",
  }) catch {
    _ => return CANARY_UNEXPECTED_ERROR
  }
  let info = operator.info() catch {
    _ => {
      operator.close()
      return CANARY_UNEXPECTED_ERROR
    }
  }
  if info.scheme != "memory" ||
    info.root != "/generic-prefix/" ||
    !info.capability.can_stat() ||
    !info.capability.can_read() ||
    !info.capability.can_write() {
    operator.close()
    return CANARY_SERVICE_INFO_MISMATCH
  }
  let payload = b"\x00\xffMoonBit\x80OpenDAL"
  operator.write("canary/binary.bin", payload) catch {
    _ => {
      operator.close()
      return CANARY_UNEXPECTED_ERROR
    }
  }
  let actual = operator.read("canary/binary.bin") catch {
    _ => {
      operator.close()
      return CANARY_UNEXPECTED_ERROR
    }
  }
  if actual != payload {
    operator.close()
    return CANARY_ROUNDTRIP_MISMATCH
  }
  let metadata = operator.stat("canary/binary.bin") catch {
    _ => {
      operator.close()
      return CANARY_UNEXPECTED_ERROR
    }
  }
  if !metadata.is_file() ||
    metadata.content_length != payload.length().to_uint64() {
    operator.close()
    return CANARY_METADATA_MISMATCH
  }
  let missing_code = try operator.read("canary/missing.bin") catch {
    error =>
      if error.is_not_found() {
        CANARY_OK
      } else {
        CANARY_WRONG_MISSING_ERROR
      }
  } noraise {
    _ => CANARY_MISSING_PATH_WAS_READ
  }
  operator.close()
  missing_code
}

///|
/// Exercise binary transfer, actual OpenDAL memory I/O, NotFound translation,
/// explicit release, and stale-slot generation reuse twice.
#export_name("opendal_mbt_wasm_canary_roundtrip")
pub fn canary_roundtrip() -> Int {
  let baseline = @opendal.live_handle_count()
  let first = run_one_lifecycle()
  if first != CANARY_OK {
    return first
  }
  let second = run_one_lifecycle()
  if second != CANARY_OK {
    return second
  }
  if @opendal.live_handle_count() != baseline {
    return CANARY_HANDLE_LEAK
  }
  CANARY_OK
}

///|
/// Round-trip exactly 16 MiB from a non-zero `BytesView` start offset.
#export_name("opendal_mbt_wasm_canary_bulk_roundtrip")
pub fn canary_bulk_roundtrip() -> Int {
  let baseline = @opendal.live_handle_count()
  let operator = @opendal.Operator::new("memory") catch {
    _ => return CANARY_UNEXPECTED_ERROR
  }
  let block = b"\x00\xffMoonBit\x80DAL!!!x"
  let storage = block.repeat(986896)
  let payload : BytesView = storage[16:16 + BULK_TRANSFER_BYTES]
  operator.write("canary/bulk.bin", payload) catch {
    _ => {
      operator.close()
      return CANARY_UNEXPECTED_ERROR
    }
  }
  let actual = operator.read("canary/bulk.bin") catch {
    _ => {
      operator.close()
      return CANARY_UNEXPECTED_ERROR
    }
  }
  let matches = actual[:] == payload
  operator.close()
  if !matches {
    return CANARY_BULK_TRANSFER_MISMATCH
  }
  if @opendal.live_handle_count() != baseline {
    return CANARY_HANDLE_LEAK
  }
  CANARY_OK
}

///|
/// Return the loaded bridge version through the public MoonBit facade.
#export_name("opendal_mbt_wasm_canary_bridge_version")
pub fn canary_bridge_version() -> UInt {
  @opendal.bridge_version()
}
