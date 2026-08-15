///|
async fn main {
  if is_node() {
    try launch_browser().wait() catch {
      error => report_node_failure("OpenDAL browser example failed: \{error}")
    } noraise {
      result => println(result)
    }
  } else {
    install_browser_failure_reporter()
    let result = try {
      let portable = run_portable_example()
      let nonblocking = verify_delayed_s3_nonblocking(browser_probe_endpoint())
      portable + "\n" + nonblocking
    } catch {
      error => "OPENDAL_BROWSER_EXAMPLE_FAILED: \{error}"
    }
    report_from_browser(result).wait() catch {
      error => fail("could not report the browser result: \{error}")
    }
  }
}
