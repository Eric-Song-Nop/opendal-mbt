///|
fn invalid_layer_argument(message : String) -> OpenDalError {
  binding_error(InvalidArgument, NewOperator, message, None, None)
}

///|
fn validate_timeout_layer(
  operation_timeout_millis : UInt64,
  io_timeout_millis : UInt64,
) -> Unit raise OpenDalError {
  if operation_timeout_millis == 0UL {
    raise invalid_layer_argument(
      "operation_timeout_millis must be greater than zero",
    )
  }
  if io_timeout_millis == 0UL {
    raise invalid_layer_argument("io_timeout_millis must be greater than zero")
  }
}

///|
fn validate_retry_layer(
  max_retries : UInt,
  min_delay_millis : UInt64,
  max_delay_millis : UInt64,
) -> Unit raise OpenDalError {
  if max_retries == 0U {
    raise invalid_layer_argument("max_retries must be greater than zero")
  }
  if min_delay_millis == 0UL {
    raise invalid_layer_argument("min_delay_millis must be greater than zero")
  }
  if max_delay_millis == 0UL {
    raise invalid_layer_argument("max_delay_millis must be greater than zero")
  }
  if min_delay_millis > max_delay_millis {
    raise invalid_layer_argument(
      "min_delay_millis must not exceed max_delay_millis",
    )
  }
}

///|
fn validate_concurrency_limit_layer(
  operation_limit : UInt,
  http_request_limit : UInt?,
) -> Unit raise OpenDalError {
  if operation_limit == 0U {
    raise invalid_layer_argument("operation_limit must be greater than zero")
  }
  match http_request_limit {
    Some(0U) =>
      raise invalid_layer_argument(
        "http_request_limit must be greater than zero when provided",
      )
    _ => ()
  }
}

///|
fn take_layered_operator(result : NativeResult) -> Operator raise OpenDalError {
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, NewOperator, None, None)
  }
  let operator = native_result_take_operator(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, NewOperator, None, None)
  }
  native_result_release(result)
  operator
}

///|
/// Return a new operator with explicit control-operation and I/O timeouts.
///
/// The original operator and resources already opened from it are unchanged.
/// When composing timeout with retry, add timeout first and retry second so
/// each retry attempt is inside the timeout budget. The native layer rejects
/// duplicate timeout layers and the unsafe reverse composition.
pub fn Operator::with_timeout(
  self : Operator,
  operation_timeout_millis~ : UInt64,
  io_timeout_millis~ : UInt64,
) -> Operator raise OpenDalError {
  validate_timeout_layer(operation_timeout_millis, io_timeout_millis)
  native_operator_with_timeout(
    self, operation_timeout_millis, io_timeout_millis,
  )
  |> take_layered_operator()
}

///|
/// Return a new operator with explicit retry policy.
///
/// `max_retries` counts attempts after the initial request. Retries are
/// opt-in and can replay stateful Writer operations after an uncertain remote
/// commit; this method does not promise exactly-once writes or safe append
/// replay. Add timeout before retry when composing both policies. The native
/// layer rejects duplicate retry layers and the unsafe reverse composition.
pub fn Operator::with_retry(
  self : Operator,
  max_retries~ : UInt,
  min_delay_millis? : UInt64 = 100UL,
  max_delay_millis? : UInt64 = 10000UL,
  jitter? : Bool = true,
) -> Operator raise OpenDalError {
  validate_retry_layer(max_retries, min_delay_millis, max_delay_millis)
  native_operator_with_retry(
    self, max_retries, min_delay_millis, max_delay_millis, jitter,
  )
  |> take_layered_operator()
}

///|
/// Return a new operator with explicit operation and optional HTTP concurrency
/// limits.
///
/// `operation_limit` bounds concurrent OpenDAL operations. For body-style
/// resources, including Reader-created streams, Writers, Listers, Deleters,
/// and Copiers, a permit remains held for the body lifetime. When provided,
/// `http_request_limit` separately bounds HTTP requests and its permit remains
/// held until the response body is dropped.
///
/// The original operator and resources already opened from it are unchanged.
/// Compose policies as timeout, then retry, then concurrency limit. This layer
/// can be added only once and is always outermost; timeout or retry cannot be
/// added afterward.
pub fn Operator::with_concurrency_limit(
  self : Operator,
  operation_limit~ : UInt,
  http_request_limit? : UInt,
) -> Operator raise OpenDalError {
  validate_concurrency_limit_layer(operation_limit, http_request_limit)
  let (has_http_request_limit, http_request_limit) = match http_request_limit {
    Some(limit) => (true, limit)
    None => (false, 0U)
  }
  native_operator_with_concurrency_limit(
    self,
    operation_limit.to_uint64(),
    has_http_request_limit,
    http_request_limit.to_uint64(),
  )
  |> take_layered_operator()
}
