///|
test "S3 auth constructors copy and preserve typed credential choices" {
  let static_auth = S3Auth::static_credentials(
    access_key_id="prefix-access-suffix"[7:13],
    secret_access_key="secret",
    session_token="token",
  )
  guard static_auth.kind
    is Credentials(
      {
        kind: StaticCredentials(
          access_key_id~,
          secret_access_key~,
          session_token=Some(session_token)
        ),
      }
    ) else {
    fail("expected static S3 credentials")
  }
  assert_eq(access_key_id, "access")
  assert_eq(secret_access_key, "secret")
  assert_eq(session_token, "token")

  let assumed = S3Auth::assume_role(
    role_arn="arn:aws:iam::123456789012:role/example",
    external_id="external",
    role_session_name="moonbit",
    duration_seconds=900UL,
  )
  guard assumed.kind
    is AssumeRole(
      role_arn~,
      source={ kind: DefaultChain(disable_ec2_metadata=false) },
      external_id=Some(external_id),
      role_session_name=Some(role_session_name),
      duration_seconds=Some(duration_seconds)
    ) else {
    fail("expected assume-role auth with a default-chain source")
  }
  assert_eq(role_arn, "arn:aws:iam::123456789012:role/example")
  assert_eq(external_id, "external")
  assert_eq(role_session_name, "moonbit")
  assert_eq(duration_seconds, 900U)
}

///|
test "S3 auth rejects empty credentials without exposing their values" {
  let supplied_secret = "do-not-copy-this-secret-into-errors"
  try
    S3Auth::static_credentials(
      access_key_id="",
      secret_access_key=supplied_secret,
    )
  catch {
    OpenDalError(info) => {
      assert_eq(info.kind, InvalidArgument)
      assert_eq(info.operation, NewOperator)
      assert_false(info.message.contains(supplied_secret))
    }
  } noraise {
    _ => fail("expected empty access_key_id to be rejected")
  }
}

///|
test "S3 assume-role duration is bounded to UInt32" {
  try
    S3Auth::assume_role(
      role_arn="arn:aws:iam::123456789012:role/example",
      duration_seconds=0x1_0000_0000UL,
    )
  catch {
    OpenDalError(info) => {
      assert_eq(info.kind, InvalidArgument)
      assert_eq(info.operation, NewOperator)
    }
  } noraise {
    _ => fail("expected an overflowing duration_seconds to be rejected")
  }
}
