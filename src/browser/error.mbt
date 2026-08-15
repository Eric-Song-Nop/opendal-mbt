///|
/// Return the complete structured context carried by this OpenDAL error.
pub fn OpenDalError::info(self : OpenDalError) -> ErrorInfo {
  match self {
    OpenDalError(info) => info
  }
}

///|
/// Recover a structured OpenDAL error from MoonBit's target-neutral `Error`
/// type. Async catch clauses receive `Error`, so this conversion preserves the
/// same inspection path on native and JavaScript targets.
pub fn OpenDalError::from_error(error : Error) -> OpenDalError? {
  match error {
    OpenDalError(info) => Some(OpenDalError(info))
    _ => None
  }
}

///|
/// Return the stable category of this OpenDAL error.
pub fn OpenDalError::kind(self : OpenDalError) -> ErrorKind {
  self.info().kind
}

///|
/// Return OpenDAL's retry-status classification for this error.
pub fn OpenDalError::status(self : OpenDalError) -> ErrorStatus {
  self.info().status
}

///|
/// Return whether OpenDAL classified this failure as temporary.
///
/// This classification alone does not make an operation safe to retry.
pub fn OpenDalError::is_temporary(self : OpenDalError) -> Bool {
  self.status() is Temporary
}

///|
/// Return whether OpenDAL classified this failure as persistent.
///
/// This classification alone does not make an operation safe to retry.
pub fn OpenDalError::is_persistent(self : OpenDalError) -> Bool {
  self.status() is Persistent
}

///|
/// Return whether this error records an operation cancelled through the
/// browser `AbortSignal` bridge.
pub fn OpenDalError::is_cancelled(self : OpenDalError) -> Bool {
  self.kind() is Cancelled
}

///|
/// Format an OpenDAL error without exposing backend configuration values.
pub impl Show for OpenDalError with fn output(self, logger) {
  let info = self.info()
  logger <+
    "OpenDAL \{Repr(info.kind)} during \{Repr(info.operation)}: \{info.message}"
}

///|
/// Construct package-internal context for translating one bridge outcome.
fn error_context(
  operation : Operation,
  path? : String,
  destination_path? : String,
) -> ErrorContext {
  { operation, path, destination_path }
}

///|
/// Translate the versioned numeric bridge error kind into the public model.
fn error_kind_from_bridge(code : Int, name : String) -> ErrorKind {
  match code {
    1 => Unexpected
    2 => Unsupported
    3 => ConfigInvalid
    4 => NotFound
    5 => PermissionDenied
    6 => IsADirectory
    7 => NotADirectory
    8 => AlreadyExists
    9 => RateLimited
    10 => IsSameFile
    11 => ConditionNotMatch
    12 => RangeNotSatisfied
    0x1001 => InvalidArgument
    0x1002 => ResourceClosed
    0x1003 => BufferTooLarge
    0x1004 => AbiMismatch
    0x1005 => Cancelled
    0x1006 => ResourceBusy
    other => UnknownKind(other, name)
  }
}

///|
/// Translate the versioned numeric bridge status into the public model.
fn error_status_from_bridge(code : Int) -> ErrorStatus {
  match code {
    1 => Permanent
    2 => Temporary
    3 => Persistent
    other => UnknownStatus(other)
  }
}

///|
/// Construct an owned error from a Promise-resolved browser bridge outcome.
///
/// Numeric values are authoritative. Unknown values and their bridge-provided
/// names are retained so a newer bridge fails open without losing diagnostics.
fn error_from_bridge(
  kind_code : Int,
  status_code : Int,
  kind_name : String,
  message : String,
  context : ErrorContext,
) -> OpenDalError {
  OpenDalError({
    kind: error_kind_from_bridge(kind_code, kind_name),
    status: error_status_from_bridge(status_code),
    message,
    operation: context.operation,
    path: context.path,
    destination_path: context.destination_path,
  })
}

///|
/// Construct a permanent package-local error at one operation context.
fn local_error(
  kind : ErrorKind,
  message : String,
  context : ErrorContext,
) -> OpenDalError {
  OpenDalError({
    kind,
    status: Permanent,
    message,
    operation: context.operation,
    path: context.path,
    destination_path: context.destination_path,
  })
}
