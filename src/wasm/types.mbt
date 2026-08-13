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
/// A portable snapshot of operations supported by one configured backend.
pub struct Capability {
  priv word0 : UInt64
  priv delete_max_size_value : UInt64
} derive(Eq, Debug)

///|
fn Capability::has(self : Capability, bit : Int) -> Bool {
  (self.word0 & (1UL << bit)) != 0UL
}

///|
pub fn Capability::can_stat(self : Capability) -> Bool {
  self.has(0)
}

///|
pub fn Capability::can_read(self : Capability) -> Bool {
  self.has(1)
}

///|
pub fn Capability::can_write(self : Capability) -> Bool {
  self.has(2)
}

///|
pub fn Capability::can_create_dir(self : Capability) -> Bool {
  self.has(3)
}

///|
pub fn Capability::can_delete(self : Capability) -> Bool {
  self.has(4)
}

///|
pub fn Capability::can_list(self : Capability) -> Bool {
  self.has(5)
}

///|
pub fn Capability::can_copy(self : Capability) -> Bool {
  self.has(6)
}

///|
pub fn Capability::can_rename(self : Capability) -> Bool {
  self.has(7)
}

///|
pub fn Capability::can_read_suffix(self : Capability) -> Bool {
  self.has(8)
}

///|
pub fn Capability::can_write_append(self : Capability) -> Bool {
  self.has(9)
}

///|
pub fn Capability::can_list_with_limit(self : Capability) -> Bool {
  self.has(10)
}

///|
pub fn Capability::can_list_with_start_after(self : Capability) -> Bool {
  self.has(11)
}

///|
pub fn Capability::can_list_recursive(self : Capability) -> Bool {
  self.has(12)
}

///|
pub fn Capability::can_presign_stat(self : Capability) -> Bool {
  self.has(13)
}

///|
pub fn Capability::can_presign_read(self : Capability) -> Bool {
  self.has(14)
}

///|
pub fn Capability::can_presign_write(self : Capability) -> Bool {
  self.has(15)
}

///|
pub fn Capability::delete_max_size(self : Capability) -> Int? {
  if self.delete_max_size_value == 0UL {
    None
  } else if self.delete_max_size_value > 2147483647UL {
    Some(2147483647)
  } else {
    Some(self.delete_max_size_value.to_int())
  }
}

///|
/// Read-only service identity and effective capability snapshot.
pub struct OperatorInfo {
  scheme : String
  root : String
  name : String
  capability : Capability
} derive(Eq, Debug)

///|
/// A generation-checked operator handle owned by the Rust Wasm bridge.
pub struct Operator {
  priv mut handle : UInt
}

///|
/// Observable state of an experimental callback operation.
pub enum OperationState {
  Pending
  Completed
  Cancelled
  Closed
} derive(Eq, Debug)

///|
/// A cancellable browser operation driven by the official loader.
///
/// This callback surface is experimental until MoonBit exposes a documented
/// continuation API for ordinary browser Wasm hosts.
pub struct Operation {
  priv mut handle : UInt
  priv mut state : OperationState
}
