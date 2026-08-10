///|
fn whole_object_contract(
  op : @opendal.Operator,
) -> Unit raise @opendal.OpenDalError {
  let written = op.write("hello.txt", Bytes::new(0))
  let content = op.read("hello.txt")
  let present = op.exists("hello.txt")
  ignore(written)
  ignore(content)
  ignore(present)
}

///|
test "contract: whole-object workflow type-checks" {
  ignore(whole_object_contract)
}

///|
fn construction_contract(
  config : Map[String, String],
) -> @opendal.Operator raise @opendal.OpenDalError {
  @opendal.Operator::new("memory", config)
}

///|
test "contract: scheme and copied config type-check" {
  ignore(construction_contract)
}
