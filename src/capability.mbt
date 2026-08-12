///|
fn Capability::has(self : Capability, bit : Int) -> Bool {
  let word = bit / 64
  let offset = bit % 64
  (native_capability_word(self, word) & (1UL << offset)) != 0UL
}

///|
fn capability_size_hint(value : UInt64) -> Int? {
  if value == 0UL {
    None
  } else if value > 2147483647UL {
    Some(2147483647)
  } else {
    Some(value.to_int())
  }
}

///|
/// Return whether metadata lookup is available through this binding.
pub fn Capability::can_stat(self : Capability) -> Bool {
  self.has(0)
}

///|
/// Return whether whole-object reads are available through this binding.
pub fn Capability::can_read(self : Capability) -> Bool {
  self.has(1)
}

///|
/// Return whether the selected backend natively supports suffix ranges.
/// The binding does not emulate suffix reads with a preliminary stat.
pub fn Capability::can_read_suffix(self : Capability) -> Bool {
  self.has(8)
}

///|
/// Return whether whole-object writes are available through this binding.
pub fn Capability::can_write(self : Capability) -> Bool {
  self.has(2)
}

///|
/// Return whether append mode is supported for writes through this binding.
pub fn Capability::can_write_append(self : Capability) -> Bool {
  self.has(9)
}

///|
/// Return whether directory creation is available through this binding.
pub fn Capability::can_create_dir(self : Capability) -> Bool {
  self.has(3)
}

///|
/// Return whether deletion is available through this binding.
pub fn Capability::can_delete(self : Capability) -> Bool {
  self.has(4)
}

///|
/// Return the backend's per-request batch-delete size hint, when available.
///
/// This is distinct from `Operator::delete_many`'s total binding input ceiling.
/// Values above MoonBit's positive Int range are reported as `Int::MAX`.
pub fn Capability::delete_max_size(self : Capability) -> Int? {
  capability_size_hint(native_capability_word(self, 1))
}

///|
/// Return whether server-side copy is available through this binding.
pub fn Capability::can_copy(self : Capability) -> Bool {
  self.has(6)
}

///|
/// Return whether native rename is available through this binding.
pub fn Capability::can_rename(self : Capability) -> Bool {
  self.has(7)
}

///|
/// Return whether prefix listing is available through this binding.
pub fn Capability::can_list(self : Capability) -> Bool {
  self.has(5)
}

///|
/// Return whether the backend accepts listing limit hints.
pub fn Capability::can_list_with_limit(self : Capability) -> Bool {
  self.has(10)
}

///|
/// Return whether the backend accepts listing start-after hints.
pub fn Capability::can_list_with_start_after(self : Capability) -> Bool {
  self.has(11)
}

///|
/// Return whether recursive prefix listing is available.
pub fn Capability::can_list_recursive(self : Capability) -> Bool {
  self.has(12)
}

///|
/// Return whether stat requests can be presigned through this binding.
pub fn Capability::can_presign_stat(self : Capability) -> Bool {
  self.has(13)
}

///|
/// Return whether read requests can be presigned through this binding.
pub fn Capability::can_presign_read(self : Capability) -> Bool {
  self.has(14)
}

///|
/// Return whether write requests can be presigned through this binding.
pub fn Capability::can_presign_write(self : Capability) -> Bool {
  self.has(15)
}
