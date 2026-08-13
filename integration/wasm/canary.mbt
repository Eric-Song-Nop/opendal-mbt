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
const ASYNC_IDLE = 0

///|
const ASYNC_WRITE_PENDING = 1

///|
const ASYNC_READ_PENDING = 2

///|
const ASYNC_STAT_PENDING = 3

///|
const ASYNC_MISSING_PENDING = 4

///|
const ASYNC_OK = 5

///|
const ASYNC_ERROR = -1

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
  let operation = operator.read_callback("canary/missing-async.bin", result => {
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
fn start_async_stat(operator : @opendal.Operator, payload : Bytes) -> Unit {
  async_stage.val = ASYNC_STAT_PENDING
  let operation = operator.stat_callback("canary/async.bin", result => {
    match result {
      Ok(metadata) if metadata.is_file &&
        metadata.content_length == payload.length().to_uint64() =>
        start_async_missing(operator)
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
  let operation = operator.read_callback("canary/async.bin", result => {
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
  async_stage.val = ASYNC_WRITE_PENDING
  let operation = operator.write_callback("canary/async.bin", payload, result => {
    match result {
      Ok(_) => start_async_read(operator, payload)
      Err(_) => fail_async_lifecycle()
    }
  }) catch {
    _ => {
      fail_async_lifecycle()
      return ASYNC_ERROR
    }
  }
  async_operation.val = Some(operation)
  ASYNC_WRITE_PENDING
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
  if !metadata.is_file ||
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
/// Return the loaded bridge version through the public MoonBit facade.
#export_name("opendal_mbt_wasm_canary_bridge_version")
pub fn canary_bridge_version() -> UInt {
  @opendal.bridge_version()
}
