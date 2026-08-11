///|
fn Capability::has(self : Capability, bit : Int) -> Bool {
  let word = bit / 64
  let offset = bit % 64
  (native_capability_word(self, word) & (1UL << offset)) != 0UL
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
/// Return whether suffix ranges are supported for reads through this binding.
pub fn Capability::can_read_suffix(self : Capability) -> Bool {
  self.has(8)
}

///|
/// Return whether whole-object writes are available through this binding.
pub fn Capability::can_write(self : Capability) -> Bool {
  self.has(2)
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
