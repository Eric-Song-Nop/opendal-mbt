///|
fn safe_presigned_headers(
  raw_headers : ArrayView[(String, Bytes)],
) -> Map[String, String] raise {
  let headers : Map[String, String] = Map([])
  let names : Map[String, Unit] = Map([])
  for raw_header in raw_headers {
    let (name, raw_value) = raw_header
    let normalized_name = name.to_lower()
    if names.contains(normalized_name) {
      raise Failure::Failure(
        "presigned S3 request contains duplicate HTTP headers",
      )
    }
    let value = try @utf8.decode(raw_value) catch {
      _ =>
        raise Failure::Failure(
          "presigned S3 request contains a non-UTF-8 HTTP header",
        )
    } noraise {
      value => value
    }
    names[normalized_name] = ()
    headers[name] = value
  }
  headers
}

///|
fn safe_presigned_method(
  request : @opendal.PresignedRequest,
) -> @http.RequestMethod raise {
  match request.http_method {
    "GET" => Get
    "HEAD" => Head
    "PUT" => Put
    _ =>
      raise Failure::Failure(
        "presigned S3 request uses an unsupported HTTP method",
      )
  }
}

///|
async fn execute_presigned(
  request : @opendal.PresignedRequest,
  body : Bytes,
) -> (Int, Bytes) {
  let request_method = safe_presigned_method(request)
  let raw_headers : Array[(String, Bytes)] = []
  let mut has_content_length = false
  for header in request.headers {
    raw_headers.push((header.name, header.value))
    if header.name.to_lower() == "content-length" {
      has_content_length = true
    }
  }
  let headers = safe_presigned_headers(raw_headers)
  if !has_content_length {
    headers["Content-Length"] = body.length().to_string()
  }
  let (response, response_body) = try {
    match request_method {
      Put => {
        // `http.request` treats its headers as connection-wide headers, where
        // Content-Length is deliberately ignored. The streaming PUT helper
        // passes request headers at the message layer, so MinIO receives a
        // fixed-size payload instead of unsupported chunked S3 upload data.
        let client = @http.put_stream(request.uri, headers~)
        defer client.close()
        let response = client..write(body).end_request()
        (response, client.read_all())
      }
      _ => @http.request(request.uri, request_method, headers, body)
    }
  } catch {
    _ => raise Failure::Failure("presigned S3 HTTP request failed")
  } noraise {
    result => result
  }
  (response.code, response_body.binary())
}

///|
fn assert_successful_presigned_response(code : Int) -> Unit raise {
  if code < 200 || code >= 300 {
    raise Failure::Failure("presigned S3 HTTP request returned status \{code}")
  }
}

///|
test "presigned S3 executor rejects duplicate header names" {
  try
    safe_presigned_headers([("x-example", b"first"), ("X-Example", b"second")])
  catch {
    Failure::Failure(message) => assert_true(message.contains("duplicate"))
    _ => fail("unexpected duplicate-header validation error")
  } noraise {
    _ => fail("expected duplicate presigned headers to be rejected")
  }
}

///|
test "presigned S3 executor rejects binary header values" {
  try safe_presigned_headers([("x-example", [0xff])]) catch {
    Failure::Failure(message) => assert_true(message.contains("non-UTF-8"))
    _ => fail("unexpected binary-header validation error")
  } noraise {
    _ => fail("expected a binary presigned header to be rejected")
  }
}

///|
async test "S3 integration: execute presigned write read and stat" {
  match s3_integration_config() {
    None => ()
    Some(config) => {
      let operator = config.operator()
      let capability = operator.info().capability
      assert_true(capability.can_presign_write())
      assert_true(capability.can_presign_read())
      assert_true(capability.can_presign_stat())
      let path = "presigned.bin"
      let payload : Bytes = [
        0x00, 0xff, 0x50, 0x52, 0x45, 0x53, 0x49, 0x47, 0x4e,
      ]

      let write_request = operator.presign_write(
        path,
        expires_in_seconds=300UL,
        content_type="application/octet-stream",
      )
      assert_eq(write_request.http_method, "PUT")
      let (write_code, _) = execute_presigned(write_request, payload)
      assert_successful_presigned_response(write_code)
      assert_eq(operator.read(path), payload)

      let read_request = operator.presign_read(path, expires_in_seconds=300UL)
      assert_eq(read_request.http_method, "GET")
      let (read_code, read_body) = execute_presigned(read_request, b"")
      assert_successful_presigned_response(read_code)
      assert_eq(read_body, payload)

      let stat_request = operator.presign_stat(path, expires_in_seconds=300UL)
      assert_eq(stat_request.http_method, "HEAD")
      let (stat_code, _) = execute_presigned(stat_request, b"")
      assert_successful_presigned_response(stat_code)
      operator.delete(path)
    }
  }
}
