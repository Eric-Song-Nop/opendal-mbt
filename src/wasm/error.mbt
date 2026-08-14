///|
/// Return the structured context carried by this OpenDAL error.
pub fn OpenDalError::info(self : OpenDalError) -> ErrorInfo {
  match self {
    OpenDalError(info) => info
  }
}

///|
/// Return the stable error category.
pub fn OpenDalError::kind(self : OpenDalError) -> ErrorKind {
  self.info().kind
}

///|
/// Return OpenDAL's retry-status classification.
pub fn OpenDalError::status(self : OpenDalError) -> ErrorStatus {
  self.info().status
}

///|
/// Return whether OpenDAL classified the failure as temporary.
/// This does not by itself make an operation safe to retry.
pub fn OpenDalError::is_temporary(self : OpenDalError) -> Bool {
  self.status() is Temporary
}

///|
/// Return whether OpenDAL classified the failure as persistent.
/// This does not by itself make an operation safe to retry.
pub fn OpenDalError::is_persistent(self : OpenDalError) -> Bool {
  self.status() is Persistent
}

///|
pub impl Show for OpenDalError with fn output(self, logger) {
  let info = self.info()
  logger <+
    "OpenDAL \{Repr(info.kind)} during \{Repr(info.operation)}: \{info.message}"
}

///|
fn error_context(
  operation : Operation,
  path? : String,
  destination_path? : String,
) -> ErrorContext {
  { operation, path, destination_path }
}

///|
fn error_from_snapshot(
  snapshot : BridgeErrorSnapshot,
  context : ErrorContext,
) -> OpenDalError {
  OpenDalError({
    kind: snapshot.kind,
    status: snapshot.status,
    message: snapshot.message,
    operation: context.operation,
    path: context.path,
    destination_path: context.destination_path,
  })
}

///|
fn local_error(
  kind : ErrorKind,
  message : String,
  context : ErrorContext,
) -> OpenDalError {
  error_from_snapshot({ kind, status: Permanent, message }, context)
}
