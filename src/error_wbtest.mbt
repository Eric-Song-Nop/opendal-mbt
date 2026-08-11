///|
fn error_with_status(status : ErrorStatus) -> OpenDalError {
  OpenDalError({
    kind: Unexpected,
    status,
    message: "status probe",
    operation: Check,
    path: None,
    destination_path: None,
  })
}

///|
test "error status helpers preserve OpenDAL classifications" {
  let temporary = error_with_status(Temporary)
  assert_true(temporary.is_temporary())
  assert_false(temporary.is_persistent())
  let persistent = error_with_status(Persistent)
  assert_false(persistent.is_temporary())
  assert_true(persistent.is_persistent())
  let permanent = error_with_status(Permanent)
  assert_false(permanent.is_temporary())
  assert_false(permanent.is_persistent())
}
