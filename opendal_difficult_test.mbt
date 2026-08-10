///|
fn random_access_contract(
  op : @opendal.Operator,
) -> Unit raise @opendal.OpenDalError {
  let options = @opendal.ReaderOptions::new(if_match="etag")
  let reader = op.reader("large.bin", options~)
  let chunk = reader.read(Range(4096UL, 1024UL))
  ignore(chunk)
  reader.close()
}

///|
fn writer_contract(op : @opendal.Operator) -> Unit raise @opendal.OpenDalError {
  let options = @opendal.WriteOptions::new(
    content_type="application/octet-stream",
  )
  let writer = op.writer("large.bin", options~)
  writer.write(Bytes::new(0))
  let metadata = writer.close()
  ignore(metadata)
}

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
    info.capability.can_read_suffix(),
  )
}

///|
fn structured_error_contract(
  error : @opendal.OpenDalError,
) -> (@opendal.ErrorKind, @opendal.Operation, String?) {
  match error {
    OpenDalError(info) => (info.kind, info.operation, info.destination_path)
  }
}

///|
test "contract: reader, writer, and materialized errors type-check" {
  ignore(random_access_contract)
  ignore(writer_contract)
  ignore(materialized_error_contract)
  ignore(readonly_value_contract)
  ignore(structured_error_contract)
}
