///|
fn assert_invalid_layer_argument(
  operation : () -> Unit raise OpenDalError,
) -> Unit raise {
  try operation() catch {
    OpenDalError(info) => {
      assert_true(info.kind is InvalidArgument)
      assert_true(info.operation is NewOperator)
      assert_eq(info.path, None)
      assert_eq(info.destination_path, None)
    }
  } noraise {
    _ => fail("expected invalid layer arguments to raise")
  }
}

///|
test "timeout layer validation requires positive budgets" {
  validate_timeout_layer(1UL, 1UL)
  assert_invalid_layer_argument(() => validate_timeout_layer(0UL, 1UL))
  assert_invalid_layer_argument(() => validate_timeout_layer(1UL, 0UL))
}

///|
test "retry layer validation requires positive ordered bounds" {
  validate_retry_layer(1U, 1UL, 1UL)
  assert_invalid_layer_argument(() => validate_retry_layer(0U, 1UL, 1UL))
  assert_invalid_layer_argument(() => validate_retry_layer(1U, 0UL, 1UL))
  assert_invalid_layer_argument(() => validate_retry_layer(1U, 1UL, 0UL))
  assert_invalid_layer_argument(() => validate_retry_layer(1U, 2UL, 1UL))
}

///|
fn layers_public_contract(operator : Operator) -> Operator raise OpenDalError {
  operator
  .with_timeout(operation_timeout_millis=10000UL, io_timeout_millis=3000UL)
  .with_retry(
    max_retries=3U,
    min_delay_millis=50UL,
    max_delay_millis=2000UL,
    jitter=true,
  )
}

///|
fn retry_defaults_contract(operator : Operator) -> Operator raise OpenDalError {
  operator.with_retry(max_retries=3U)
}

///|
test "layer public facade type-checks" {
  ignore(layers_public_contract)
  ignore(retry_defaults_contract)
}
