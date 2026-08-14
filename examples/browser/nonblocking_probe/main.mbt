///|
fn required_endpoint() -> String raise {
  guard @env.get_env_var("OPENDAL_MBT_PROBE_ENDPOINT") is Some(endpoint) &&
    endpoint.length() > 0 else {
    raise Failure::Failure(
      "OPENDAL_MBT_PROBE_ENDPOINT must name the delayed local S3 fixture",
    )
  }
  endpoint
}

///|
async fn main {
  let storage = @opendal.AsyncOperator::new("s3", config={
    "bucket": "probe-bucket",
    "region": "us-east-1",
    "endpoint": required_endpoint(),
    "access_key_id": "probe-access-key",
    "secret_access_key": "probe-secret-key",
    "disable_config_load": "true",
    "disable_ec2_metadata": "true",
  })
  defer storage.close()

  let heartbeat = Ref(false)
  @async.with_task_group(group => {
    group.spawn_bg(() => {
      @async.sleep(50)
      heartbeat.val = true
    })

    try storage.read("delayed-missing.txt") catch {
      error => {
        if !heartbeat.val {
          fail(
            "native async read blocked the MoonBit scheduler until I/O completed",
          )
        }
        guard @opendal.OpenDalError::from_error(error) is Some(_) else {
          fail("delayed S3 response did not produce an OpenDalError")
        }
      }
    } noraise {
      _ => fail("the delayed S3 fixture unexpectedly returned an object")
    }
  })

  println(
    "OPENDAL_NATIVE_NONBLOCKING_OK: heartbeat ran before delayed I/O completed",
  )
}
