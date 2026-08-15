///|
async fn main {
  println(run_portable_example())
  match @env.get_env_var("OPENDAL_MBT_PROBE_ENDPOINT") {
    Some(endpoint) if endpoint.length() > 0 =>
      println(verify_delayed_s3_nonblocking(endpoint))
    _ => ()
  }
}
