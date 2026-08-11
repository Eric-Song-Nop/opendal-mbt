///|
fn materialized_error_contract(
  op : @opendal.Operator,
) -> Result[Bytes, @opendal.OpenDalError] {
  try op.read("missing.bin") catch {
    error => Err(error)
  } noraise {
    data => Ok(data)
  }
}

///|
fn readonly_value_contract(
  entry : @opendal.Entry,
  info : @opendal.OperatorInfo,
) -> (@opendal.EntryMode, UInt64, String, Bool, Bool) {
  (
    entry.metadata.mode,
    entry.metadata.content_length,
    info.scheme,
    info.capability.can_read(),
    info.capability.can_list(),
  )
}

///|
fn structured_error_contract(
  error : @opendal.OpenDalError,
) -> (@opendal.ErrorKind, @opendal.Operation, String?) {
  let info = error.info()
  (error.kind(), info.operation, info.destination_path)
}

///|
fn random_reader_contract(
  op : @opendal.Operator,
  range : @opendal.ByteRange,
) -> @opendal.Reader raise @opendal.OpenDalError {
  let reader = op.open_reader(
    "value.bin",
    version="version",
    if_match="etag",
    if_none_match="other-etag",
  )
  let bytes = reader.read(range)
  let ranged = op.read("value.bin", range~, version="version")
  ignore(bytes)
  ignore(ranged)
  reader
}

///|
test "contract: snapshots and materialized errors type-check" {
  ignore(materialized_error_contract)
  ignore(readonly_value_contract)
  ignore(structured_error_contract)
  ignore(random_reader_contract)
}
