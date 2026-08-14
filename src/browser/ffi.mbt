///|
#external
priv type JsRuntime

///|
#external
priv type JsOperator

///|
#external
priv type JsReadStream

///|
#external
priv type JsWriter

///|
#external
priv type JsLister

///|
#external
priv type JsMetadata

///|
#external
priv type JsEntry

///|
#external
priv type JsOperatorInfo

///|
#external
priv type JsCapability

///|
#external
priv type JsErrorInfo

///|
#external
priv type JsOutcome

///|
extern "js" fn load_js_runtime(
  runtime_module_url : String,
  bridge_module_url : String,
  bridge_wasm_url : String,
) -> @js_async.Promise[JsRuntime] =
  #| async (runtimeModuleUrl, bridgeModuleUrl, bridgeWasmUrl) => {
  #|   const [{ loadOpenDalBrowser }, { default: initializeBridge }] =
  #|     await Promise.all([import(runtimeModuleUrl), import(bridgeModuleUrl)])
  #|   const bridge = await initializeBridge({ module_or_path: bridgeWasmUrl })
  #|   return loadOpenDalBrowser({ bridge })
  #| }

///|
extern "js" fn JsOutcome::is_ok(self : JsOutcome) -> Bool =
  #| (outcome) => outcome?.ok === true

///|
extern "js" fn JsOutcome::operator_value(self : JsOutcome) -> JsOperator =
  #| (outcome) => outcome.value

///|
extern "js" fn JsOutcome::operator_info_value(
  self : JsOutcome,
) -> JsOperatorInfo =
  #| (outcome) => outcome.value

///|
extern "js" fn JsOutcome::schemes_value(self : JsOutcome) -> Array[String] =
  #| (outcome) => outcome.value

///|
extern "js" fn JsOutcome::metadata_value(self : JsOutcome) -> JsMetadata =
  #| (outcome) => outcome.value

///|
extern "js" fn JsOutcome::bytes_value(self : JsOutcome) -> Bytes =
  #| (outcome) => outcome.value

///|
extern "js" fn JsOutcome::entries_value(self : JsOutcome) -> Array[JsEntry] =
  #| (outcome) => outcome.value

///|
extern "js" fn JsOutcome::has_value(self : JsOutcome) -> Bool =
  #| (outcome) => outcome.value !== undefined

///|
extern "js" fn JsOutcome::read_stream_value(self : JsOutcome) -> JsReadStream =
  #| (outcome) => outcome.value

///|
extern "js" fn JsOutcome::writer_value(self : JsOutcome) -> JsWriter =
  #| (outcome) => outcome.value

///|
extern "js" fn JsOutcome::lister_value(self : JsOutcome) -> JsLister =
  #| (outcome) => outcome.value

///|
extern "js" fn JsOutcome::entry_value(self : JsOutcome) -> JsEntry =
  #| (outcome) => outcome.value

///|
extern "js" fn JsOutcome::error(self : JsOutcome) -> JsErrorInfo =
  #| (outcome) => outcome.error

///|
extern "js" fn JsErrorInfo::kind_code(self : JsErrorInfo) -> Int =
  #| (error) => error.kindCode | 0

///|
extern "js" fn JsErrorInfo::status_code(self : JsErrorInfo) -> Int =
  #| (error) => error.statusCode | 0

///|
extern "js" fn JsErrorInfo::kind_name(self : JsErrorInfo) -> String =
  #| (error) => error.kind

///|
extern "js" fn JsErrorInfo::message(self : JsErrorInfo) -> String =
  #| (error) => error.message

///|
extern "js" fn JsRuntime::available_schemes(self : JsRuntime) -> JsOutcome =
  #| (runtime) => runtime.availableSchemes()

///|
extern "js" fn JsRuntime::operator(
  self : JsRuntime,
  scheme : String,
  keys : Array[String],
  values : Array[String],
) -> JsOutcome =
  #| (runtime, scheme, keys, values) => {
  #|   const config = Object.create(null)
  #|   for (let i = 0; i < keys.length; i += 1) config[keys[i]] = values[i]
  #|   return runtime.operator(scheme, config)
  #| }

///|
extern "js" fn JsRuntime::close(self : JsRuntime) -> JsOutcome =
  #| (runtime) => runtime.close()

///|
extern "js" fn JsOperator::info(self : JsOperator) -> JsOutcome =
  #| (operator) => operator.info()

///|
extern "js" fn JsOperator::close(self : JsOperator) -> JsOutcome =
  #| (operator) => operator.close()

///|
extern "js" fn JsOperatorInfo::scheme(self : JsOperatorInfo) -> String =
  #| (info) => info.scheme

///|
extern "js" fn JsOperatorInfo::root(self : JsOperatorInfo) -> String =
  #| (info) => info.root

///|
extern "js" fn JsOperatorInfo::name(self : JsOperatorInfo) -> String =
  #| (info) => info.name

///|
extern "js" fn JsOperatorInfo::capability(
  self : JsOperatorInfo,
) -> JsCapability =
  #| (info) => info.capability

///|
extern "js" fn JsCapability::word0(self : JsCapability) -> UInt64 =
  #| (capability) => capability.word0

///|
extern "js" fn JsCapability::has_delete_max_size(self : JsCapability) -> Bool =
  #| (capability) => capability.deleteMaxSize !== undefined

///|
extern "js" fn JsCapability::delete_max_size(self : JsCapability) -> UInt64 =
  #| (capability) => capability.deleteMaxSize

///|
extern "js" fn JsMetadata::mode(self : JsMetadata) -> Int =
  #| (metadata) => metadata.mode === "File" ? 1 :
  #|   metadata.mode === "Directory" ? 2 : 0

///|
extern "js" fn JsMetadata::content_length(self : JsMetadata) -> UInt64 =
  #| (metadata) => metadata.contentLength

///|
extern "js" fn JsMetadata::has_is_current(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.isCurrent !== undefined

///|
extern "js" fn JsMetadata::is_current(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.isCurrent

///|
extern "js" fn JsMetadata::is_deleted(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.isDeleted

///|
extern "js" fn JsMetadata::has_cache_control(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.cacheControl !== undefined

///|
extern "js" fn JsMetadata::cache_control(self : JsMetadata) -> String =
  #| (metadata) => metadata.cacheControl

///|
extern "js" fn JsMetadata::has_content_disposition(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.contentDisposition !== undefined

///|
extern "js" fn JsMetadata::content_disposition(self : JsMetadata) -> String =
  #| (metadata) => metadata.contentDisposition

///|
extern "js" fn JsMetadata::has_content_encoding(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.contentEncoding !== undefined

///|
extern "js" fn JsMetadata::content_encoding(self : JsMetadata) -> String =
  #| (metadata) => metadata.contentEncoding

///|
extern "js" fn JsMetadata::has_content_md5(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.contentMd5 !== undefined

///|
extern "js" fn JsMetadata::content_md5(self : JsMetadata) -> String =
  #| (metadata) => metadata.contentMd5

///|
extern "js" fn JsMetadata::has_content_type(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.contentType !== undefined

///|
extern "js" fn JsMetadata::content_type(self : JsMetadata) -> String =
  #| (metadata) => metadata.contentType

///|
extern "js" fn JsMetadata::has_etag(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.etag !== undefined

///|
extern "js" fn JsMetadata::etag(self : JsMetadata) -> String =
  #| (metadata) => metadata.etag

///|
extern "js" fn JsMetadata::has_last_modified(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.lastModified !== undefined

///|
extern "js" fn JsMetadata::last_modified_seconds(self : JsMetadata) -> Int64 =
  #| (metadata) => metadata.lastModified.unixSeconds

///|
extern "js" fn JsMetadata::last_modified_nanoseconds(self : JsMetadata) -> Int =
  #| (metadata) => metadata.lastModified.nanoseconds | 0

///|
extern "js" fn JsMetadata::has_version(self : JsMetadata) -> Bool =
  #| (metadata) => metadata.version !== undefined

///|
extern "js" fn JsMetadata::version(self : JsMetadata) -> String =
  #| (metadata) => metadata.version

///|
extern "js" fn JsEntry::path(self : JsEntry) -> String =
  #| (entry) => entry.path

///|
extern "js" fn JsEntry::name(self : JsEntry) -> String =
  #| (entry) => entry.name

///|
extern "js" fn JsEntry::metadata(self : JsEntry) -> JsMetadata =
  #| (entry) => entry.metadata

///|
extern "js" fn JsOperator::create_dir(
  self : JsOperator,
  path : String,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, path, signal) => operator.createDir(path, { signal })

///|
extern "js" fn JsOperator::write(
  self : JsOperator,
  path : String,
  data : Bytes,
  append : Bool,
  has_content_type : Bool,
  content_type : String,
  has_content_disposition : Bool,
  content_disposition : String,
  has_content_encoding : Bool,
  content_encoding : String,
  has_cache_control : Bool,
  cache_control : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, path, data, append,
  #|  hasContentType, contentType,
  #|  hasContentDisposition, contentDisposition,
  #|  hasContentEncoding, contentEncoding,
  #|  hasCacheControl, cacheControl,
  #|  hasIfMatch, ifMatch, hasIfNoneMatch, ifNoneMatch, signal) => {
  #|   const options = { append, signal }
  #|   if (hasContentType) options.contentType = contentType
  #|   if (hasContentDisposition) options.contentDisposition = contentDisposition
  #|   if (hasContentEncoding) options.contentEncoding = contentEncoding
  #|   if (hasCacheControl) options.cacheControl = cacheControl
  #|   if (hasIfMatch) options.ifMatch = ifMatch
  #|   if (hasIfNoneMatch) options.ifNoneMatch = ifNoneMatch
  #|   return operator.write(path, data, options)
  #| }

///|
extern "js" fn JsOperator::read(
  self : JsOperator,
  path : String,
  range_kind : Int,
  range_offset : UInt64,
  range_length : UInt64,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, path, rangeKind, rangeOffset, rangeLength,
  #|  hasVersion, version, hasIfMatch, ifMatch,
  #|  hasIfNoneMatch, ifNoneMatch, signal) => {
  #|   const kinds = ["Full", "From", "Range", "Suffix"]
  #|   const range = { kind: kinds[rangeKind] }
  #|   if (rangeKind === 1 || rangeKind === 2) range.offset = rangeOffset
  #|   if (rangeKind === 2 || rangeKind === 3) range.length = rangeLength
  #|   const options = { range, signal }
  #|   if (hasVersion) options.version = version
  #|   if (hasIfMatch) options.ifMatch = ifMatch
  #|   if (hasIfNoneMatch) options.ifNoneMatch = ifNoneMatch
  #|   return operator.read(path, options)
  #| }

///|
extern "js" fn JsOperator::stat(
  self : JsOperator,
  path : String,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, path, hasVersion, version, hasIfMatch, ifMatch,
  #|  hasIfNoneMatch, ifNoneMatch, signal) => {
  #|   const options = { signal }
  #|   if (hasVersion) options.version = version
  #|   if (hasIfMatch) options.ifMatch = ifMatch
  #|   if (hasIfNoneMatch) options.ifNoneMatch = ifNoneMatch
  #|   return operator.stat(path, options)
  #| }

///|
extern "js" fn JsOperator::delete(
  self : JsOperator,
  path : String,
  has_version : Bool,
  version : String,
  recursive : Bool,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, path, hasVersion, version, recursive, signal) => {
  #|   const options = { recursive, signal }
  #|   if (hasVersion) options.version = version
  #|   return operator.delete(path, options)
  #| }

///|
extern "js" fn JsOperator::list(
  self : JsOperator,
  path : String,
  recursive : Bool,
  has_limit : Bool,
  limit : UInt64,
  has_start_after : Bool,
  start_after : String,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, path, recursive, hasLimit, limit,
  #|  hasStartAfter, startAfter, signal) => {
  #|   const options = { recursive, signal }
  #|   if (hasLimit) options.limit = limit
  #|   if (hasStartAfter) options.startAfter = startAfter
  #|   return operator.list(path, options)
  #| }

///|
extern "js" fn JsOperator::copy(
  self : JsOperator,
  source : String,
  destination : String,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, source, destination, signal) =>
  #|   operator.copy(source, destination, { signal })

///|
extern "js" fn JsOperator::rename(
  self : JsOperator,
  source : String,
  destination : String,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, source, destination, signal) =>
  #|   operator.rename(source, destination, { signal })

///|
extern "js" fn JsOperator::open_read_stream(
  self : JsOperator,
  path : String,
  range_kind : Int,
  range_offset : UInt64,
  range_length : UInt64,
  chunk_size : Int,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, path, rangeKind, rangeOffset, rangeLength, chunkSize,
  #|  hasVersion, version, hasIfMatch, ifMatch,
  #|  hasIfNoneMatch, ifNoneMatch, signal) => {
  #|   const kinds = ["Full", "From", "Range", "Suffix"]
  #|   const range = { kind: kinds[rangeKind] }
  #|   if (rangeKind === 1 || rangeKind === 2) range.offset = rangeOffset
  #|   if (rangeKind === 2 || rangeKind === 3) range.length = rangeLength
  #|   const options = { range, chunkSize, signal }
  #|   if (hasVersion) options.version = version
  #|   if (hasIfMatch) options.ifMatch = ifMatch
  #|   if (hasIfNoneMatch) options.ifNoneMatch = ifNoneMatch
  #|   return operator.openReadStream(path, options)
  #| }

///|
extern "js" fn JsReadStream::next(
  self : JsReadStream,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (stream, signal) => stream.next({ signal })

///|
extern "js" fn JsReadStream::close(self : JsReadStream) -> JsOutcome =
  #| (stream) => stream.close()

///|
extern "js" fn JsOperator::open_writer(
  self : JsOperator,
  path : String,
  append : Bool,
  has_content_type : Bool,
  content_type : String,
  has_content_disposition : Bool,
  content_disposition : String,
  has_content_encoding : Bool,
  content_encoding : String,
  has_cache_control : Bool,
  cache_control : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, path, append,
  #|  hasContentType, contentType,
  #|  hasContentDisposition, contentDisposition,
  #|  hasContentEncoding, contentEncoding,
  #|  hasCacheControl, cacheControl,
  #|  hasIfMatch, ifMatch, hasIfNoneMatch, ifNoneMatch, signal) => {
  #|   const options = { append, signal }
  #|   if (hasContentType) options.contentType = contentType
  #|   if (hasContentDisposition) options.contentDisposition = contentDisposition
  #|   if (hasContentEncoding) options.contentEncoding = contentEncoding
  #|   if (hasCacheControl) options.cacheControl = cacheControl
  #|   if (hasIfMatch) options.ifMatch = ifMatch
  #|   if (hasIfNoneMatch) options.ifNoneMatch = ifNoneMatch
  #|   return operator.openWriter(path, options)
  #| }

///|
extern "js" fn JsWriter::write(
  self : JsWriter,
  chunk : Bytes,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (writer, chunk, signal) => writer.write(chunk, { signal })

///|
extern "js" fn JsWriter::finish(
  self : JsWriter,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (writer, signal) => writer.finish({ signal })

///|
extern "js" fn JsWriter::abort(
  self : JsWriter,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (writer, signal) => writer.abort({ signal })

///|
extern "js" fn JsWriter::close(self : JsWriter) -> JsOutcome =
  #| (writer) => writer.close()

///|
extern "js" fn JsOperator::open_lister(
  self : JsOperator,
  path : String,
  recursive : Bool,
  has_limit : Bool,
  limit : UInt64,
  has_start_after : Bool,
  start_after : String,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (operator, path, recursive, hasLimit, limit,
  #|  hasStartAfter, startAfter, signal) => {
  #|   const options = { recursive, signal }
  #|   if (hasLimit) options.limit = limit
  #|   if (hasStartAfter) options.startAfter = startAfter
  #|   return operator.openLister(path, options)
  #| }

///|
extern "js" fn JsLister::next(
  self : JsLister,
  signal : @js_async.AbortSignal,
) -> @js_async.Promise[JsOutcome] =
  #| (lister, signal) => lister.next({ signal })

///|
extern "js" fn JsLister::close(self : JsLister) -> JsOutcome =
  #| (lister) => lister.close()
