///|
/// This branch intentionally defines only the MoonBit facade and private FFI
/// contract. The unreachable body keeps every public async call shape under
/// compiler control independently of runtime behavior tests.
async test "Phase 5E async facade contract compiles" {
  if false {
    let operator = Operator::new("memory")
    let async_operator = operator.as_async()
    async_operator.check()
    let exists : Bool = async_operator.exists("object")
    let stat_metadata : Metadata = async_operator.stat(
      "object",
      version="version",
      if_match="etag",
      if_none_match="other-etag",
    )
    let write_metadata : Metadata = async_operator.write(
      "object",
      b"payload",
      append=true,
      content_type="application/octet-stream",
      content_disposition="attachment",
      content_encoding="identity",
      cache_control="no-cache",
      if_match="etag",
      if_none_match="other-etag",
    )
    async_operator.create_dir("prefix/")
    async_operator.delete("prefix/", version="version", recursive=true)
    let entries : Array[Entry] = async_operator.list(
      "prefix/",
      recursive=true,
      limit=10UL,
      start_after="prefix/previous",
    )
    let copied : Metadata = async_operator.copy("object", "copied")
    async_operator.rename("copied", "renamed")
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
    ignore(exists)
    ignore(stat_metadata)
    ignore(write_metadata)
    ignore(entries)
    ignore(copied)
    ignore(chunk)
    ignore(metadata)
  }
}
