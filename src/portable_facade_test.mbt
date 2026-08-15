///|
/// This black-box contract is deliberately compiled unchanged for native and
/// JavaScript. It guards the portable package path, constructor, whole-object
/// data path, and bounded streaming API against target drift.
async test "portable facade: identical memory data path" {
  let operator = @opendal.AsyncOperator::new("memory")
  defer operator.close()
  let payload = b"portable opendal"
  let metadata = operator.write("portable/whole.bin", payload)
  assert_eq(metadata.content_length, payload.length().to_uint64())
  assert_eq(operator.read("portable/whole.bin"), payload)
  assert_eq(
    operator.read("portable/whole.bin", range=Range(offset=9UL, length=7UL)),
    b"opendal",
  )
  let writer = operator.open_writer("portable/stream.bin")
  writer.write(b"portable ")
  writer.write(b"opendal")
  writer.finish() |> ignore
  let stream = operator.open_read_stream("portable/stream.bin", chunk_size=5)
  let chunks = Buffer(size_hint=payload.length())
  while stream.next() is Some(chunk) {
    chunks.write_bytes(chunk)
  }
  stream.close()
  stream.close()
  assert_eq(chunks.to_bytes(), payload)
  let closed_stream = operator.open_read_stream("portable/stream.bin")
  closed_stream.close()
  closed_stream.close()
  try closed_stream.next() catch {
    error =>
      match @opendal.OpenDalError::from_error(error) {
        Some(error) => assert_true(error.kind() is ResourceClosed)
        None => fail("portable stream close returned an unexpected error type")
      }
  } noraise {
    _ => fail("portable stream remained usable after close")
  }
  let aborted = operator.open_writer("portable/aborted.bin")
  aborted.write(b"discard")
  aborted.abort()
  aborted.abort()
  operator.close()
  operator.close()
  try operator.read("portable/whole.bin") catch {
    error =>
      match @opendal.OpenDalError::from_error(error) {
        Some(error) => assert_true(error.kind() is ResourceClosed)
        None => fail("portable close returned an unexpected error type")
      }
  } noraise {
    _ => fail("portable operator remained usable after close")
  }
}

///|
async test "portable facade: read-stream transfer bound" {
  let operator = @opendal.AsyncOperator::new("memory")
  defer operator.close()
  let transfer_limit = 256 * 1024
  let payload = Bytes::make(transfer_limit + 1, 0x5a)
  operator.write("portable/bound.bin", payload) |> ignore
  let stream = operator.open_read_stream("portable/bound.bin")
  guard stream.next() is Some(first) else {
    fail("portable stream ended before the first bounded chunk")
  }
  assert_true(first.length() > 0)
  assert_true(first.length() <= transfer_limit)
  let mut received = first.length()
  while stream.next() is Some(chunk) {
    assert_true(chunk.length() <= transfer_limit)
    received += chunk.length()
  }
  assert_eq(received, payload.length())
  stream.close()
  try
    operator.open_read_stream(
      "portable/bound.bin",
      chunk_size=transfer_limit + 1,
    )
  catch {
    error =>
      match @opendal.OpenDalError::from_error(error) {
        Some(error) => assert_true(error.kind() is InvalidArgument)
        None =>
          fail("portable chunk validation returned an unexpected error type")
      }
  } noraise {
    _ => fail("portable stream accepted a chunk above 256 KiB")
  }
  let writer = operator.open_writer("portable/writer-bound.bin")
  try writer.write(Bytes::make(transfer_limit + 1, 0x5a)) catch {
    error =>
      match @opendal.OpenDalError::from_error(error) {
        Some(error) => assert_true(error.kind() is BufferTooLarge)
        None => fail("portable writer returned an unexpected error type")
      }
  } noraise {
    _ => fail("portable writer accepted a chunk above 256 KiB")
  }
  writer.abort()
}
