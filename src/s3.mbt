///|
/// Credentials that may be used directly or as the source for an S3
/// assume-role request. Values are copied when the source is constructed.
pub struct S3CredentialSource {
  priv kind : S3CredentialSourceKind
}

///|
priv enum S3CredentialSourceKind {
  DefaultChain(disable_ec2_metadata~ : Bool)
  StaticCredentials(
    access_key_id~ : String,
    secret_access_key~ : String,
    session_token~ : String?
  )
}

///|
/// Authentication policy for an S3 operator. Credential material is opaque
/// and has no debug or display representation.
pub struct S3Auth {
  priv kind : S3AuthKind
}

///|
priv enum S3AuthKind {
  Credentials(S3CredentialSource)
  Unsigned
  AssumeRole(
    role_arn~ : String,
    source~ : S3CredentialSource,
    external_id~ : String?,
    role_session_name~ : String?,
    duration_seconds~ : UInt?
  )
}

///|
fn s3_invalid_argument(message : String) -> OpenDalError {
  binding_error(InvalidArgument, NewOperator, message, None, None)
}

///|
fn s3_required_text(
  value : StringView,
  message : String,
) -> String raise OpenDalError {
  let value = value.to_owned()
  if value.length() == 0 {
    raise s3_invalid_argument(message)
  }
  value
}

///|
fn s3_optional_text(
  value : StringView?,
  message : String,
) -> String? raise OpenDalError {
  match value {
    Some(value) => Some(s3_required_text(value, message))
    None => None
  }
}

///|
fn s3_duration_seconds(value : UInt64?) -> UInt? raise OpenDalError {
  match value {
    Some(value) => {
      if value > 0xffff_ffffUL {
        raise s3_invalid_argument(
          "S3 assume-role duration_seconds exceeds the UInt32 ceiling",
        )
      }
      Some(value.to_uint())
    }
    None => None
  }
}

///|
/// Use the standard AWS credential chain. EC2 instance metadata remains
/// available unless explicitly disabled.
pub fn S3CredentialSource::default_chain(
  disable_ec2_metadata? : Bool = false,
) -> S3CredentialSource {
  { kind: DefaultChain(disable_ec2_metadata~) }
}

///|
/// Use an explicit access-key pair, with an optional session token.
pub fn S3CredentialSource::static_credentials(
  access_key_id~ : StringView,
  secret_access_key~ : StringView,
  session_token? : StringView,
) -> S3CredentialSource raise OpenDalError {
  let access_key_id = s3_required_text(
    access_key_id, "S3 access_key_id must not be empty",
  )
  let secret_access_key = s3_required_text(
    secret_access_key, "S3 secret_access_key must not be empty",
  )
  let session_token = s3_optional_text(
    session_token, "S3 session_token must not be empty when provided",
  )
  {
    kind: StaticCredentials(access_key_id~, secret_access_key~, session_token~),
  }
}

///|
/// Use the standard AWS credential chain. EC2 instance metadata remains
/// available unless explicitly disabled.
pub fn S3Auth::default_chain(disable_ec2_metadata? : Bool = false) -> S3Auth {
  {
    kind: Credentials(S3CredentialSource::default_chain(disable_ec2_metadata~)),
  }
}

///|
/// Use an explicit access-key pair, with an optional session token.
pub fn S3Auth::static_credentials(
  access_key_id~ : StringView,
  secret_access_key~ : StringView,
  session_token? : StringView,
) -> S3Auth raise OpenDalError {
  {
    kind: Credentials(
      S3CredentialSource::static_credentials(
        access_key_id~,
        secret_access_key~,
        session_token?,
      ),
    ),
  }
}

///|
/// Send unsigned S3 requests. This is intended for public buckets or custom
/// endpoints that explicitly allow anonymous access.
pub fn S3Auth::unsigned() -> S3Auth {
  { kind: Unsigned }
}

///|
/// Assume an IAM role using either the standard credential chain or explicit
/// static source credentials. Omitting `source` selects the standard chain.
pub fn S3Auth::assume_role(
  role_arn~ : StringView,
  source? : S3CredentialSource,
  external_id? : StringView,
  role_session_name? : StringView,
  duration_seconds? : UInt64,
) -> S3Auth raise OpenDalError {
  let role_arn = s3_required_text(role_arn, "S3 role_arn must not be empty")
  let source = match source {
    Some(source) => source
    None => S3CredentialSource::default_chain()
  }
  let external_id = s3_optional_text(
    external_id, "S3 external_id must not be empty when provided",
  )
  let role_session_name = s3_optional_text(
    role_session_name, "S3 role_session_name must not be empty when provided",
  )
  let duration_seconds = s3_duration_seconds(duration_seconds)
  {
    kind: AssumeRole(
      role_arn~,
      source~,
      external_id~,
      role_session_name~,
      duration_seconds~,
    ),
  }
}
