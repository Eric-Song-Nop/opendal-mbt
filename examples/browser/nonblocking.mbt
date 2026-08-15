///|
// Prove that an OpenDAL request which is still waiting on external I/O does
// not block MoonBit's async scheduler. Both native and browser entry points
// call this exact function against their own delayed local S3 fixture.
async fn verify_delayed_s3_nonblocking(endpoint : String) -> String {
  if endpoint.length() == 0 {
    fail("the delayed local S3 fixture endpoint is missing")
  }
  let storage = @opendal.AsyncOperator::new("s3", config={
    "bucket": "probe-bucket",
    "region": "us-east-1",
    "endpoint": endpoint,
    "access_key_id": "probe-access-key",
    "secret_access_key": "probe-secret-key",
    "disable_config_load": "true",
    "disable_ec2_metadata": "true",
  })
  defer storage.close()

  let heartbeat_armed = Ref(false)
  let heartbeat_ran = Ref(false)
  @async.with_task_group(group => {
    group.spawn_bg(() => {
      // Publish that the timer has been scheduled before the read begins.
      heartbeat_armed.val = true
      @async.sleep(50)
      heartbeat_ran.val = true
    })
    while !heartbeat_armed.val {
      @async.pause()
    }

    try storage.read("delayed-missing.txt") catch {
      error => {
        match @opendal.OpenDalError::from_error(error) {
          Some(storage_error) =>
            if !(storage_error.kind() is NotFound) {
              fail(
                "delayed S3 fixture did not return NotFound: \{storage_error}",
              )
            }
          None => fail("delayed S3 response did not produce an OpenDalError")
        }
        if !heartbeat_ran.val {
          fail("OpenDAL async read blocked the scheduler while I/O was pending")
        }
      }
    } noraise {
      _ => fail("the delayed S3 fixture unexpectedly returned an object")
    }
  })

  "OPENDAL_ASYNC_IO_NONBLOCKING_OK: heartbeat ran before delayed S3 I/O completed"
}
