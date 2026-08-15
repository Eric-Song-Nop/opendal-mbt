///|
/// Internal marker used when the compatibility browser package is compiled as
/// a dependency of the native root facade.
#doc(hidden)
pub let unimplemented : Unit = ()

///|
#warnings("-alert_unimplemented")
let _keep_js_imports_target_neutral : Unit = {
  ignore(@async.sleep)
  ignore(@js_async.AbortController::new)
}
