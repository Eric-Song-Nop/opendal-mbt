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
priv struct S3NativeSource {
  kind : UInt
  disable_ec2_metadata : Bool
  access_key_id : String
  secret_access_key : String
  has_session_token : Bool
  session_token : String
}

///|
priv struct S3NativeAuth {
  kind : UInt
  source : S3NativeSource
  role_arn : String
  has_external_id : Bool
  external_id : String
  has_role_session_name : Bool
  role_session_name : String
  has_duration_seconds : Bool
  duration_seconds : UInt
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
fn s3_native_optional_text(value : String?) -> (Bool, String) {
  match value {
    Some(value) => (true, value)
    None => (false, "")
  }
}

///|
fn S3CredentialSource::native_parts(
  self : S3CredentialSource,
) -> S3NativeSource {
  match self.kind {
    DefaultChain(disable_ec2_metadata~) =>
      {
        kind: 0U,
        disable_ec2_metadata,
        access_key_id: "",
        secret_access_key: "",
        has_session_token: false,
        session_token: "",
      }
    StaticCredentials(access_key_id~, secret_access_key~, session_token~) => {
      let (has_session_token, session_token) = s3_native_optional_text(
        session_token,
      )
      {
        kind: 1U,
        disable_ec2_metadata: false,
        access_key_id,
        secret_access_key,
        has_session_token,
        session_token,
      }
    }
  }
}

///|
fn S3Auth::native_parts(self : S3Auth) -> S3NativeAuth {
  match self.kind {
    Credentials(source) => {
      let source = source.native_parts()
      {
        kind: source.kind,
        source,
        role_arn: "",
        has_external_id: false,
        external_id: "",
        has_role_session_name: false,
        role_session_name: "",
        has_duration_seconds: false,
        duration_seconds: 0U,
      }
    }
    Unsigned =>
      {
        kind: 2U,
        source: S3CredentialSource::default_chain().native_parts(),
        role_arn: "",
        has_external_id: false,
        external_id: "",
        has_role_session_name: false,
        role_session_name: "",
        has_duration_seconds: false,
        duration_seconds: 0U,
      }
    AssumeRole(
      role_arn~,
      source~,
      external_id~,
      role_session_name~,
      duration_seconds~
    ) => {
      let (has_external_id, external_id) = s3_native_optional_text(external_id)
      let (has_role_session_name, role_session_name) = s3_native_optional_text(
        role_session_name,
      )
      let (has_duration_seconds, duration_seconds) = match duration_seconds {
        Some(value) => (true, value)
        None => (false, 0U)
      }
      {
        kind: 3U,
        source: source.native_parts(),
        role_arn,
        has_external_id,
        external_id,
        has_role_session_name,
        role_session_name,
        has_duration_seconds,
        duration_seconds,
      }
    }
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

///|
/// Construct an S3 operator with a typed, secret-safe authentication policy.
/// Omitting `auth` selects the standard AWS credential chain.
pub fn Operator::s3(
  bucket : StringView,
  region~ : StringView,
  root? : StringView,
  endpoint? : StringView,
  auth? : S3Auth,
  virtual_host_style? : Bool = false,
) -> Operator raise OpenDalError {
  let bucket = s3_required_text(bucket, "S3 bucket must not be empty")
  let region = s3_required_text(region, "S3 region must not be empty")
  let root = s3_optional_text(root, "S3 root must not be empty when provided")
  let endpoint = s3_optional_text(
    endpoint, "S3 endpoint must not be empty when provided",
  )
  let auth = match auth {
    Some(auth) => auth
    None => S3Auth::default_chain()
  }
  let auth = auth.native_parts()
  let (has_root, root) = s3_native_optional_text(root)
  let (has_endpoint, endpoint) = s3_native_optional_text(endpoint)
  let result = native_operator_s3(
    bucket,
    region,
    has_root,
    root,
    has_endpoint,
    endpoint,
    auth.kind,
    auth.source.kind,
    auth.source.disable_ec2_metadata,
    auth.source.access_key_id,
    auth.source.secret_access_key,
    auth.source.has_session_token,
    auth.source.session_token,
    auth.role_arn,
    auth.has_external_id,
    auth.external_id,
    auth.has_role_session_name,
    auth.role_session_name,
    auth.has_duration_seconds,
    auth.duration_seconds,
    virtual_host_style,
  )
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
