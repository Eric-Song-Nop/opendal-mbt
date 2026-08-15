///|
/// The JavaScript target keeps the public package path identical to native
/// while the compatibility `/browser` package continues to own the established
/// browser implementation during the migration.
let _keep_native_imports_target_neutral : Unit = {
  ignore(@async.sleep)
  ignore(@pipe.unimplemented)
}

///|
pub using @browser {
  type AsyncLister,
  type AsyncOperator,
  type AsyncReadStream,
  type AsyncWriter,
  type BrowserAssets,
  type ByteRange,
  type Capability,
  type Entry,
  type EntryMode,
  type ErrorInfo,
  type ErrorKind,
  type ErrorStatus,
  type Metadata,
  type OpenDalError,
  type Operation,
  type Operator,
  type OperatorInfo,
  type Runtime,
  type Timestamp,
}
