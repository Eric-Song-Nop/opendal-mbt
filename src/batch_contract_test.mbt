///|
fn batch_delete_contract(
  operator : @opendal.Operator,
  paths : ArrayView[String],
) -> Unit raise @opendal.OpenDalError {
  let _ : Int? = operator.info().capability.delete_max_size()
  operator.delete_many(paths)
}

///|
test "contract: Phase 5D batch facade type-checks" {
  ignore(batch_delete_contract)
}
