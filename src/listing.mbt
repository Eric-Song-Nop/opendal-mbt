///|
/// Open a fallible prefix-listing cursor. The returned resource should be
/// closed when iteration stops before end-of-stream.
pub fn Operator::open_lister(
  self : Operator,
  path : StringView,
  recursive? : Bool = false,
  limit? : UInt64,
  start_after? : StringView,
) -> Lister raise OpenDalError {
  let path = path.to_owned()
  let has_limit = limit is Some(_)
  let limit = limit.unwrap_or(0UL)
  let (has_start_after, start_after) = normalize_optional_text(start_after)
  let result = native_operator_lister(
    self, path, recursive, has_limit, limit, has_start_after, start_after,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, List, Some(path), None)
  }
  let native = native_result_take_lister(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, List, Some(path), None)
  }
  native_result_release(result)
  { native, path }
}

///|
/// Return the next entry, None at end-of-stream, or raise on failure.
/// End-of-stream is stable. An I/O failure is terminal.
pub fn Lister::next(self : Lister) -> Entry? raise OpenDalError {
  let result = native_lister_next(self.native)
  match native_result_status(result) {
    0U => {
      let entry = native_result_take_entry(result)
      if native_result_status(result) != 0U {
        self.close()
        raise error_from_native_result(result, List, Some(self.path), None)
      }
      native_result_release(result)
      let converted = try entry_from_native(entry, self.path) catch {
        error => {
          self.close()
          raise error
        }
      } noraise {
        value => value
      }
      Some(converted)
    }
    1U => {
      native_result_release(result)
      None
    }
    _ => {
      self.close()
      raise error_from_native_result(result, List, Some(self.path), None)
    }
  }
}

///|
/// Release listing resources early. Closing more than once is harmless.
pub fn Lister::close(self : Lister) -> Unit {
  native_lister_close(self.native)
}

///|
/// Materialize a prefix listing into memory. The optional limit is a backend
/// request hint rather than a total-result bound.
pub fn Operator::list(
  self : Operator,
  path : StringView,
  recursive? : Bool = false,
  limit? : UInt64,
  start_after? : StringView,
) -> Array[Entry] raise OpenDalError {
  let path = path.to_owned()
  let lister = self.open_lister(path, recursive~, limit?, start_after?)
  let entries : Array[Entry] = []
  try {
    for ;; {
      match lister.next() {
        Some(entry) => {
          if entries.length() == 2147483647 {
            raise binding_error(
              BufferTooLarge,
              List,
              "list has too many entries to materialize in a MoonBit Array",
              Some(path),
              None,
            )
          }
          entries.push(entry)
        }
        None => break
      }
    }
    lister.close()
    entries
  } catch {
    error => {
      lister.close()
      raise error
    }
  } noraise {
    value => value
  }
}
