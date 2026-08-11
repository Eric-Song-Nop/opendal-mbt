///|
fn listing_contract(op : @opendal.Operator) -> Unit raise @opendal.OpenDalError {
  let lister = op.open_lister(
    "prefix/",
    recursive=true,
    limit=128UL,
    start_after="prefix/old",
  )
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
  op.delete("prefix/", recursive=true)
}

///|
test "contract: recursive deletion is an option" {
  ignore(recursive_delete_contract)
}
