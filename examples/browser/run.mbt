///|
// Both the native executable and the real-browser executable call this exact
// function. It intentionally stays inside OpenDAL's portable async surface.
async fn run_portable_example() -> String {
  // This is a deterministic scheduler smoke signal, not a latency benchmark:
  // both tasks must yield and resume on MoonBit's async runtime.
  let scheduler_signal = @async.all([
    () => {
      @async.pause()
      "left"
    },
    () => {
      @async.pause()
      "right"
    },
  ])
  if scheduler_signal.length() != 2 ||
    scheduler_signal[0] != "left" ||
    scheduler_signal[1] != "right" {
    fail("async scheduler smoke check failed")
  }

  let storage = @opendal.AsyncOperator::new("memory")
  defer storage.close()
  let direct_path = "portable-example/direct.txt"
  let streamed_path = "portable-example/streamed.txt"
  let direct_payload = b"hello from portable async"
  let streamed_payload = b"hello from MoonBit async"

  let direct_metadata = storage.write(
    direct_path,
    direct_payload,
    content_type="text/plain",
  )
  if direct_metadata.content_length != direct_payload.length().to_uint64() {
    fail("write returned an unexpected content length")
  }
  if storage.read(direct_path) != direct_payload {
    fail("read did not return the bytes that were written")
  }

  let writer = storage.open_writer(streamed_path, content_type="text/plain")
  let streamed_metadata = try {
    writer.write(b"hello from ")
    writer.write(b"MoonBit async")
    writer.finish()
  } catch {
    error => {
      writer.abort() catch {
        _ => ()
      }
      raise error
    }
  }
  if streamed_metadata.content_length != streamed_payload.length().to_uint64() {
    fail("streaming writer returned an unexpected content length")
  }

  let stream = storage.open_read_stream(streamed_path, chunk_size=7)
  defer stream.close()
  let streamed_output = Buffer(size_hint=streamed_payload.length())
  while stream.next() is Some(chunk) {
    streamed_output.write_bytes(chunk)
  }
  stream.close()
  let streamed_result = streamed_output.to_bytes()
  if streamed_result != streamed_payload {
    fail("streaming reader did not return the bytes that were written")
  }
  let streamed_bytes = streamed_result.length()
  "OPENDAL_PORTABLE_ASYNC_OK: \{streamed_bytes} streamed bytes, scheduler=2"
}
