///|
async test "native async memory: bounded read and stream preserve ranges" {
  let operator = @opendal.Operator::new("memory")
  operator.write("async/read.bin", b"0123456789") |> ignore
  let async_operator = operator.as_async()
  assert_eq(
    async_operator.read("async/read.bin", range=Range(offset=2UL, length=5UL)),
    b"23456",
  )
  assert_eq(
    async_operator.read("async/read.bin", range=Suffix(length=3UL)),
    b"789",
  )
  let stream = async_operator.open_read_stream(
    "async/read.bin",
    range=From(offset=1UL),
    chunk_size=3,
  )
  let collected : Array[Byte] = []
  for ;; {
    match stream.next() {
      Some(chunk) => {
        assert_true(chunk.length() <= 3)
        for byte in chunk {
          collected.push(byte)
        }
      }
      None => break
    }
  }
  assert_eq(Bytes::from_array(collected), b"123456789")
  assert_eq(stream.next(), None)
  assert_eq(stream.next(), None)
  stream.close()
  stream.close()
}

///|
async test "native async memory: writer finish and abort are explicit" {
  let operator = @opendal.Operator::new("memory")
  let async_operator = operator.as_async()
  let writer = async_operator.open_writer(
    "async/writer.bin",
    content_type="application/octet-stream",
  )
  let first_storage = b"__moonbit-__"
  writer.write(first_storage[2:10])
  writer.write(b"writer")
  let metadata = writer.finish()
  assert_true(metadata.is_file())
  assert_eq(metadata.content_length, 14UL)
  assert_eq(metadata.content_type, Some("application/octet-stream"))
  assert_eq(operator.read("async/writer.bin"), b"moonbit-writer")

  let aborted_path = "async/aborted.bin"
  let aborted = async_operator.open_writer(aborted_path)
  aborted.write(b"discard-me")
  aborted.abort()
  aborted.abort()
  assert_false(operator.exists(aborted_path))
}
