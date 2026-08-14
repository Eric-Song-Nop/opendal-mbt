///|
/// Return true when metadata describes a file.
pub fn Metadata::is_file(self : Metadata) -> Bool {
  self.mode is File
}

///|
/// Return true when metadata describes a directory.
pub fn Metadata::is_dir(self : Metadata) -> Bool {
  self.mode is Directory
}

///|
/// Return true when this listed entry describes a file.
pub fn Entry::is_file(self : Entry) -> Bool {
  self.metadata.is_file()
}

///|
/// Return true when this listed entry describes a directory.
pub fn Entry::is_dir(self : Entry) -> Bool {
  self.metadata.is_dir()
}
