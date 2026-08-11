///|
/// A managed, same-Operator copy of one object.
///
/// This resource is neither recursive nor cross-service. Its finalizer only
/// frees native state; it never reports completion or successful abort.
pub struct Copier {
  priv native : NativeCopier
  priv source : String
  priv destination : String
}

///|
/// Open an incremental copy between two paths on this Operator.
///
/// The returned Copier owns the native operator state it needs and may outlive
/// this Operator.
pub fn Operator::open_copier(
  self : Operator,
  source : StringView,
  destination : StringView,
) -> Copier raise OpenDalError {
  let source = source.to_owned()
  let destination = destination.to_owned()
  let result = native_operator_copier(self, source, destination)
  if native_result_status(result) != 0U {
    raise error_from_native_result(
      result,
      Copy,
      Some(source),
      Some(destination),
    )
  }
  let native = native_result_take_copier(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(
      result,
      Copy,
      Some(source),
      Some(destination),
    )
  }
  native_result_release(result)
  { native, source, destination }
}

///|
/// Advance one upstream copy step.
///
/// `Some(bytes)` is that step's byte-count delta. `None` is a stable completed
/// marker until `finish` consumes the retained destination metadata. A native
/// failure is terminal.
pub fn Copier::next(self : Copier) -> UInt64? raise OpenDalError {
  let result = native_copier_next(self.native)
  match native_result_status(result) {
    0U => {
      let bytes = native_result_take_uint64(result)
      if native_result_status(result) != 0U {
        raise error_from_native_result(
          result,
          Copy,
          Some(self.source),
          Some(self.destination),
        )
      }
      native_result_release(result)
      Some(bytes)
    }
    1U => {
      native_result_release(result)
      None
    }
    _ =>
      raise error_from_native_result(
        result,
        Copy,
        Some(self.source),
        Some(self.destination),
      )
  }
}

///|
/// Drive any remaining copy steps and return destination metadata.
///
/// The first finish failure is terminal.
pub fn Copier::finish(self : Copier) -> Metadata raise OpenDalError {
  let result = native_copier_finish(self.native)
  if native_result_status(result) != 0U {
    raise error_from_native_result(
      result,
      Copy,
      Some(self.source),
      Some(self.destination),
    )
  }
  let metadata = native_result_take_metadata(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(
      result,
      Copy,
      Some(self.source),
      Some(self.destination),
    )
  }
  native_result_release(result)
  metadata_from_native(
    metadata,
    Copy,
    Some(self.source),
    destination_path=self.destination,
  )
}

///|
/// Abort an unfinished copy.
///
/// A successful abort is idempotent. Aborting a completed, finished, or
/// failed Copier raises `ResourceClosed`; successful abort does not promise
/// rollback of remote effects already made visible.
pub fn Copier::abort(self : Copier) -> Unit raise OpenDalError {
  let result = native_copier_abort(self.native)
  if native_result_status(result) != 0U {
    raise error_from_native_result(
      result,
      Copy,
      Some(self.source),
      Some(self.destination),
    )
  }
  native_result_release(result)
}
