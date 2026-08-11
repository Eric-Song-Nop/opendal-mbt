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
test "concurrency layer validation requires positive limits" {
  validate_concurrency_limit_layer(1U, None)
  validate_concurrency_limit_layer(1U, Some(1U))
  assert_invalid_layer_argument(() => validate_concurrency_limit_layer(0U, None))
  assert_invalid_layer_argument(() => {
    validate_concurrency_limit_layer(1U, Some(0U))
  })
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
  .with_concurrency_limit(operation_limit=16U, http_request_limit=8U)
}

///|
fn retry_defaults_contract(operator : Operator) -> Operator raise OpenDalError {
  operator.with_retry(max_retries=3U)
}

///|
fn concurrency_defaults_contract(
  operator : Operator,
) -> Operator raise OpenDalError {
  operator.with_concurrency_limit(operation_limit=8U)
}

///|
test "layer public facade type-checks" {
  ignore(layers_public_contract)
  ignore(retry_defaults_contract)
  ignore(concurrency_defaults_contract)
}

///|
test "layered memory operator preserves info and shared storage" {
  let base = Operator::new("memory")
  base.write("layers/before.txt", b"before") |> ignore
  let layered = base
    .with_timeout(operation_timeout_millis=5000UL, io_timeout_millis=2000UL)
    .with_retry(
      max_retries=2U,
      min_delay_millis=1UL,
      max_delay_millis=5UL,
      jitter=false,
    )
    .with_concurrency_limit(operation_limit=4U, http_request_limit=2U)
  let info = layered.info()
  assert_eq(info.scheme, "memory")
  assert_true(info.capability.can_read())
  assert_eq(layered.read("layers/before.txt"), b"before")
  layered.write("layers/after.txt", b"after") |> ignore
  assert_eq(base.read("layers/after.txt"), b"after")
}

///|
test "native layer order and duplicate rules stay explicit" {
  let base = Operator::new("memory")
  let timeout = base.with_timeout(
    operation_timeout_millis=5000UL,
    io_timeout_millis=2000UL,
  )
  assert_invalid_layer_argument(() => {
    timeout.with_timeout(
      operation_timeout_millis=5000UL,
      io_timeout_millis=2000UL,
    )
    |> ignore
  })
  let retry = base.with_retry(
    max_retries=1U,
    min_delay_millis=1UL,
    max_delay_millis=5UL,
    jitter=false,
  )
  assert_invalid_layer_argument(() => {
    retry.with_timeout(
      operation_timeout_millis=5000UL,
      io_timeout_millis=2000UL,
    )
    |> ignore
  })
  let limited = retry.with_concurrency_limit(operation_limit=2U)
  assert_invalid_layer_argument(() => {
    limited.with_concurrency_limit(operation_limit=2U) |> ignore
  })
  assert_invalid_layer_argument(() => {
    limited.with_timeout(
      operation_timeout_millis=5000UL,
      io_timeout_millis=2000UL,
    )
    |> ignore
  })
  assert_invalid_layer_argument(() => {
    limited.with_retry(
      max_retries=1U,
      min_delay_millis=1UL,
      max_delay_millis=5UL,
      jitter=false,
    )
    |> ignore
  })
}
