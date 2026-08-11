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
