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
fn run_one_lifecycle() -> Int {
  let operator = @opendal.Operator::memory() catch {
    _ => return CANARY_UNEXPECTED_ERROR
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
