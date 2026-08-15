///|
/// Return the structured context carried by this OpenDAL error.
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
