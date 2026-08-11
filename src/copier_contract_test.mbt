///|
fn copier_progress_contract(
  operator : @opendal.Operator,
) -> UInt64? raise @opendal.OpenDalError {
  let copier = operator.open_copier("source.bin", "destination.bin")
  copier.next()
}

///|
fn copier_finish_contract(
  operator : @opendal.Operator,
) -> @opendal.Metadata raise @opendal.OpenDalError {
  operator.open_copier("source.bin", "destination.bin").finish()
}

///|
fn copier_abort_contract(
  operator : @opendal.Operator,
) -> Unit raise @opendal.OpenDalError {
  operator.open_copier("source.bin", "destination.bin").abort()
}

///|
test "contract: Phase 5D Copier facade type-checks" {
  ignore(copier_progress_contract)
  ignore(copier_finish_contract)
  ignore(copier_abort_contract)
}
