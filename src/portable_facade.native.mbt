///|
/// Keep the compatibility browser implementation available to the JS target
/// without making native callers depend on any JavaScript symbol.
let _keep_browser_dependency_target_neutral : Unit = ignore(
  @browser.unimplemented,
)
