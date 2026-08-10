///|
fn listing_contract(op : @opendal.Operator) -> Unit raise @opendal.OpenDalError {
  let options = @opendal.ListOptions::new(
    recursive=true,
    limit=128UL,
    start_after="prefix/old",
  )
  let lister = op.lister("prefix/", options~)
  let next : @opendal.Entry? = lister.next()
  ignore(next)
  lister.close()
}

///|
test "contract: fallible streaming listing type-checks" {
  ignore(listing_contract)
}

///|
fn recursive_delete_contract(
  op : @opendal.Operator,
) -> Unit raise @opendal.OpenDalError {
  let options = @opendal.DeleteOptions::new(recursive=true)
  op.delete("prefix/", options~)
}

///|
test "contract: recursive deletion is an option" {
  ignore(recursive_delete_contract)
}
