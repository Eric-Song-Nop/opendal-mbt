///|
async fn run_browser_demo() -> String {
  let asynchronous = @opendal.AsyncOperator::new("memory")
  let payload = b"moonbit browser opendal"
  let writer = asynchronous.open_writer("demo/source.bin")
  writer.write(b"moonbit ")
  writer.write(b"browser opendal")
  let metadata = writer.finish()
  if metadata.content_length != payload.length().to_uint64() {
    fail("streaming writer returned the wrong content length")
  }
  if asynchronous.read("demo/source.bin") != payload {
    fail("browser payload did not round trip")
  }
  let stream = asynchronous.open_read_stream("demo/source.bin", chunk_size=5)
  let mut byte_count = 0
  while stream.next() is Some(chunk) {
    byte_count += chunk.length()
  }
  if byte_count != payload.length() {
    fail("streaming reader returned the wrong byte count")
  }
  let lister = asynchronous.open_lister("demo/", recursive=true)
  let mut entry_count = 0
  let mut found_source = false
  while lister.next() is Some(entry) {
    entry_count += 1
    if entry.path == "demo/source.bin" {
      found_source = true
    }
  }
  if !found_source {
    fail("streaming lister did not return the written object")
  }
  asynchronous.delete("demo/source.bin")
  asynchronous.close()
  "opendal browser demo ok: \{byte_count} bytes, \{entry_count} entry"
}

///|
async fn main {
  if is_node() {
    try launch_chrome().wait() catch {
      error =>
        report_node_failure("could not run the OpenDAL browser demo: \{error}")
    } noraise {
      result => println(result)
    }
  } else {
    install_browser_failure_reporter()
    let result = try run_browser_demo() catch {
      _ => "opendal browser demo failed: OpenDalError"
    } noraise {
      result => result
    }
    report_from_browser(result).wait() catch {
      error => fail("could not report the browser result: \{error}")
    }
  }
}
