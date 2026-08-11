///|
struct S3IntegrationConfig {
  endpoint : String
  bucket : String
  region : String
  root : String
  access_key_id : String
  secret_access_key : String
}

///|
fn required_integration_env(name : String) -> String raise {
  guard @env.get_env_var(name) is Some(value) else {
    raise Failure::Failure("missing required S3 integration setting: {name}")
  }
  if value.length() == 0 {
    raise Failure::Failure("empty required S3 integration setting: {name}")
  }
  value
}

///|
fn s3_integration_config() -> S3IntegrationConfig? raise {
  if @env.get_env_var("OPENDAL_MBT_S3_INTEGRATION") != Some("1") {
    return None
  }
  Some({
    endpoint: required_integration_env("OPENDAL_MBT_S3_ENDPOINT"),
    bucket: required_integration_env("OPENDAL_MBT_S3_BUCKET"),
    region: required_integration_env("OPENDAL_MBT_S3_REGION"),
    root: required_integration_env("OPENDAL_MBT_S3_ROOT"),
    access_key_id: required_integration_env("OPENDAL_MBT_S3_ACCESS_KEY_ID"),
    secret_access_key: required_integration_env(
      "OPENDAL_MBT_S3_SECRET_ACCESS_KEY",
    ),
  })
}

///|
fn S3IntegrationConfig::operator(
  self : S3IntegrationConfig,
) -> @opendal.Operator raise {
  let auth = @opendal.S3Auth::static_credentials(
    access_key_id=self.access_key_id,
    secret_access_key=self.secret_access_key,
  )
  @opendal.Operator::s3(
    self.bucket,
    region=self.region,
    root=self.root,
    endpoint=self.endpoint,
    auth~,
  )
}

///|
fn contains_path(entries : Array[@opendal.Entry], path : String) -> Bool {
  entries.search_by(fn(entry) { entry.path == path }) is Some(_)
}

///|
fn collect_stream(
  stream : @opendal.ReadStream,
  chunk_size : Int,
) -> Bytes raise {
  let collected : Array[Byte] = []
  for ;; {
    match stream.next() {
      Some(chunk) => {
        assert_true(chunk.length() <= chunk_size)
        for byte in chunk {
          collected.push(byte)
        }
      }
      None => break
    }
  }
  assert_eq(stream.next(), None)
  stream.close()
  Bytes::from_array(collected)
}

///|
fn finish_copier(copier : @opendal.Copier) -> @opendal.Metadata raise {
  let mut steps = 0
  for ;; {
    match copier.next() {
      Some(_) => {
        steps += 1
        if steps > 1024 {
          raise Failure::Failure(
            "S3 Copier exceeded the integration step limit",
          )
        }
      }
      None => break
    }
  }
  copier.finish()
}

///|
test "S3 integration: blocking facade exercises MinIO end to end" {
  match s3_integration_config() {
    None => ()
    Some(config) => {
      let operator = config.operator()
      operator.check()
      let capability = operator.info().capability
      assert_true(capability.can_stat())
      assert_true(capability.can_read())
      assert_true(capability.can_write())
      assert_true(capability.can_delete())
      assert_true(capability.can_list())
      assert_true(capability.can_copy())

      let binary_payload : Bytes = [
        0x00, 0x01, 0x7f, 0x80, 0xff, 0x10, 0x20, 0x30, 0x00, 0x42,
      ]
      let whole_metadata = operator.write(
        "whole.bin",
        binary_payload,
        content_type="application/octet-stream",
      )
      assert_eq(
        whole_metadata.content_length,
        binary_payload.length().to_uint64(),
      )
      assert_eq(operator.read("whole.bin"), binary_payload)
      let stat = operator.stat("whole.bin")
      assert_true(stat.is_file())
      assert_eq(stat.content_length, binary_payload.length().to_uint64())

      operator.write("stream.bin", b"0123456789abcdef") |> ignore
      let stream = operator.open_read_stream(
        "stream.bin",
        range=Range(offset=2UL, length=11UL),
        chunk_size=3,
      )
      assert_eq(collect_stream(stream, 3), b"23456789abc")

      let writer = operator.open_writer(
        "writer-finished.bin",
        content_type="application/octet-stream",
      )
      writer.write(b"chunked-")
      writer.write(b"writer")
      let writer_metadata = writer.finish()
      assert_eq(writer_metadata.content_length, 14UL)
      assert_eq(operator.read("writer-finished.bin"), b"chunked-writer")

      let aborted_writer = operator.open_writer("writer-aborted.bin")
      aborted_writer.write(b"must-not-commit")
      aborted_writer.abort()
      assert_false(operator.exists("writer-aborted.bin"))

      operator.write("listing/one.bin", b"one") |> ignore
      operator.write("listing/nested/two.bin", b"two") |> ignore
      let entries = operator.list("listing/", recursive=true)
      assert_true(contains_path(entries, "listing/one.bin"))
      assert_true(contains_path(entries, "listing/nested/two.bin"))

      operator.write("batch/one.bin", b"one") |> ignore
      operator.write("batch/two.bin", b"two") |> ignore
      operator.delete_many(["batch/one.bin", "batch/two.bin"])
      assert_false(operator.exists("batch/one.bin"))
      assert_false(operator.exists("batch/two.bin"))

      operator.write("copy/source.bin", binary_payload) |> ignore
      let copy_metadata = operator.copy("copy/source.bin", "copy/one-shot.bin")
      assert_eq(
        copy_metadata.content_length,
        binary_payload.length().to_uint64(),
      )
      assert_eq(operator.read("copy/one-shot.bin"), binary_payload)

      let copier = operator.open_copier("copy/source.bin", "copy/managed.bin")
      let copier_metadata = finish_copier(copier)
      assert_eq(
        copier_metadata.content_length,
        binary_payload.length().to_uint64(),
      )
      assert_eq(operator.read("copy/managed.bin"), binary_payload)

      operator.delete_many([
        "whole.bin", "stream.bin", "writer-finished.bin", "listing/one.bin", "listing/nested/two.bin",
        "copy/source.bin", "copy/one-shot.bin", "copy/managed.bin",
      ])
    }
  }
}
