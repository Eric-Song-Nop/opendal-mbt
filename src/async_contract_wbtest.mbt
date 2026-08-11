///|
/// This branch intentionally defines only the MoonBit facade and private FFI
/// contract. The unreachable body keeps every public async call shape under
/// compiler control independently of runtime behavior tests.
async test "Phase 5E async facade contract compiles" {
  if false {
    let operator = Operator::new("memory")
    let async_operator = operator.as_async()
    let bytes : Bytes = async_operator.read(
      "object",
      range=Range(offset=1UL, length=2UL),
      version="version",
      if_match="etag",
      if_none_match="other-etag",
    )
    let stream : AsyncReadStream = async_operator.open_read_stream(
      "object",
      range=From(offset=1UL),
      chunk_size=4096,
    )
    let chunk : Bytes? = stream.next()
    stream.close()
    let writer : AsyncWriter = async_operator.open_writer(
      "destination",
      content_type="application/octet-stream",
    )
    writer.write(bytes)
    let metadata : Metadata = writer.finish()
    writer.abort()
    ignore(chunk)
    ignore(metadata)
  }
}
