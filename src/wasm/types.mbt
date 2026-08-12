///|
/// A stable error snapshot copied out of the Rust Wasm bridge.
pub suberror WasmError {
  WasmError(Int, String)
} derive(Eq, Debug)

///|
/// Return the stable bridge error code.
pub fn WasmError::code(self : WasmError) -> Int {
  match self {
    WasmError(code, _) => code
  }
}

///|
/// Return the owned diagnostic message supplied by the bridge.
pub fn WasmError::message(self : WasmError) -> String {
  match self {
    WasmError(_, message) => message
  }
}

///|
/// Return true when OpenDAL reported that the requested path does not exist.
pub fn WasmError::is_not_found(self : WasmError) -> Bool {
  self.code() == 7
}

///|
/// The metadata subset proven by the first OpenDAL memory-service slice.
pub struct Metadata {
  content_length : UInt64
  is_file : Bool
} derive(Eq, Debug)

///|
/// A generation-checked operator handle owned by the Rust Wasm bridge.
pub struct Operator {
  priv mut handle : UInt
}
