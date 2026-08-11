///|
fn delete_path_utf8_length(path : String) -> UInt64 raise OpenDalError {
  let length = path.length()
  for index = 0, bytes = 0UL; index < length; {
    let code_unit = path[index]
    if code_unit <= 0x007f {
      continue index + 1, bytes + 1UL
    } else if code_unit <= 0x07ff {
      continue index + 1, bytes + 2UL
    } else if code_unit >= 0xd800 && code_unit <= 0xdbff {
      if index + 1 >= length {
        raise binding_error(
          InvalidArgument,
          Delete,
          "batch delete path contains invalid UTF-16",
          Some(path),
          None,
        )
      }
      let low = path[index + 1]
      if low < 0xdc00 || low > 0xdfff {
        raise binding_error(
          InvalidArgument,
          Delete,
          "batch delete path contains invalid UTF-16",
          Some(path),
          None,
        )
      }
      continue index + 2, bytes + 4UL
    } else if code_unit >= 0xdc00 && code_unit <= 0xdfff {
      raise binding_error(
        InvalidArgument,
        Delete,
        "batch delete path contains invalid UTF-16",
        Some(path),
        None,
      )
    } else {
      continue index + 1, bytes + 3UL
    }
  } nobreak {
    bytes
  }
}

///|
fn prepare_delete_many_paths(
  paths : ArrayView[String],
) -> FixedArray[String] raise OpenDalError {
  let copied : Array[String] = []
  for path in paths {
    if path.length() == 0 {
      raise binding_error(
        InvalidArgument,
        Delete,
        "batch delete paths must not be empty",
        Some(path),
        None,
      )
    }
    ignore(delete_path_utf8_length(path))
    copied.push(path)
  }
  FixedArray::from_array(copied)
}

///|
/// Delete multiple paths through OpenDAL's high-level deleter.
///
/// The binding copies and validates all paths before starting. Empty input
/// succeeds without calling the backend. Native ABI representability checks
/// and the backend's batch hint remain distinct from public API policy. A
/// returned error can follow partial remote effects; this all-or-error API
/// does not report per-path outcomes or promise input ordering, uniqueness, or
/// atomicity.
pub fn Operator::delete_many(
  self : Operator,
  paths : ArrayView[String],
) -> Unit raise OpenDalError {
  if paths.length() != 0 {
    let paths = prepare_delete_many_paths(paths)
    let result = native_operator_delete_many(self, paths)
    if native_result_status(result) != 0U {
      raise error_from_native_result(result, Delete, None, None)
    }
    native_result_release(result)
  }
}
