#include "../../native/include/opendal_mbt.h"

#include <stddef.h>
#include <string.h>

#if defined(__cplusplus)
#define ABI_STATIC_ASSERT(condition, message) static_assert(condition, message)
#else
#define ABI_STATIC_ASSERT(condition, message) _Static_assert(condition, message)
#endif

ABI_STATIC_ASSERT(sizeof(opendal_mbt_status_t) == 4,
                  "transport status must be 32-bit");
ABI_STATIC_ASSERT(OPENDAL_MBT_ABI_V1_MAJOR == 1 &&
                      OPENDAL_MBT_ABI_V1_MINOR == 8,
                  "core async parity requires ABI v1.8");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_bool_t) == 4,
                  "ABI boolean must be 32-bit");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_bytes_view_v1_t) == 16,
                  "byte view layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_kv_v1_t) == 32,
                  "key/value layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_byte_range_v1_t) == 32,
                  "byte range layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_read_options_v1_t) == 96,
                  "read options layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_reader_options_v1_t) == 64,
                  "reader options layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_read_stream_options_v1_t) == 104,
                  "read stream options layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_write_options_v1_t) == 120,
                  "write options layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_stat_options_v1_t) == 64,
                  "stat options layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_list_options_v1_t) == 48,
                  "list options layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_delete_options_v1_t) == 40,
                  "delete options layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_s3_options_v1_t) == 200,
                  "S3 options layout drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_s3_options_v1_t, auth_kind) == 24,
                  "S3 auth kind offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_s3_options_v1_t, bucket) == 32,
                  "S3 bucket offset drifted");
ABI_STATIC_ASSERT(
    offsetof(opendal_mbt_s3_options_v1_t, assume_role_duration_seconds) == 192,
    "S3 duration offset drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_timestamp_v1_t) == 16,
                  "timestamp layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_capability_v1_t) == 32,
                  "capability layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_metadata_view_v1_t) == 168,
                  "metadata view layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_entry_view_v1_t) == 48,
                  "entry view layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_operator_info_view_v1_t) == 96,
                  "operator info view layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_error_view_v1_t) == 48,
                  "error view layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_library_info_view_v1_t) == 64,
                  "library info view layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_presigned_request_view_v1_t) == 56,
                  "presigned request view layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_presigned_header_view_v1_t) == 48,
                  "presigned header view layout drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, struct_size) == 0,
                  "bootstrap size must be first");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, requested_major) == 4,
                  "bootstrap major must be second");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, library_struct_size) == 8,
                  "library output prefix drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, feature_bits) == 24,
                  "feature bit layout drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_PREFIX_SIZE == 40,
                  "bootstrap output prefix drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_INPUT_SIZE == 8,
                  "bootstrap input prefix drifted");
#if UINTPTR_MAX == UINT64_MAX
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, operator_s3) == 368,
                  "v1.2 function table offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, operator_presign_read) == 376,
                  "v1.3 function table offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, operator_with_timeout) == 424,
                  "v1.4 timeout function offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, operator_with_retry) == 432,
                  "v1.4 retry function offset drifted");
ABI_STATIC_ASSERT(
    offsetof(opendal_mbt_api_v1_t, operator_with_concurrency_limit) == 440,
    "v1.5 concurrency-limit function offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, operator_delete_many) == 448,
                  "v1.6 batch delete function offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, operator_copier) == 456,
                  "v1.6 Copier constructor offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, copier_next) == 464,
                  "v1.6 Copier next offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, copier_finish) == 472,
                  "v1.6 Copier finish offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, copier_abort) == 480,
                  "v1.6 Copier abort offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, copier_free) == 488,
                  "v1.6 Copier free offset drifted");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t,
                           async_operator_read_start) == 496,
                  "v1.7 async group must start at the v1.6 table tail");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, async_task_free) == 624,
                  "v1.7 async task free offset drifted");
ABI_STATIC_ASSERT(
    offsetof(opendal_mbt_api_v1_t, async_operator_check_start) == 632,
    "v1.8 core async group must start at the v1.7 table tail");
ABI_STATIC_ASSERT(offsetof(opendal_mbt_api_v1_t, async_task_take_lister) == 712,
                  "v1.8 async lister take offset drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_api_v1_t) == 720,
                  "v1.8 function table size drifted");
#endif
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(library_info) <=
                      OPENDAL_MBT_API_V1_FIELD_END(error_view),
                  "function table order drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(read_stream_free) <=
                      OPENDAL_MBT_API_V1_FIELD_END(writer_abort),
                  "v1.1 append order drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(writer_abort) <=
                      OPENDAL_MBT_API_V1_FIELD_END(operator_s3),
                  "v1.2 append order drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(operator_s3) <=
                      OPENDAL_MBT_API_V1_FIELD_END(operator_presign_read),
                  "presign append order drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(presigned_request_header_view) <=
                      OPENDAL_MBT_API_V1_FIELD_END(presigned_request_free),
                  "presign view group order drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(presigned_request_free) <=
                      OPENDAL_MBT_API_V1_FIELD_END(operator_with_timeout),
                  "v1.4 timeout append order drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(operator_with_timeout) <=
                      OPENDAL_MBT_API_V1_FIELD_END(operator_with_retry),
                  "v1.4 retry append order drifted");
ABI_STATIC_ASSERT(
    OPENDAL_MBT_API_V1_FIELD_END(operator_with_retry) <=
        OPENDAL_MBT_API_V1_FIELD_END(operator_with_concurrency_limit),
    "v1.5 concurrency-limit append order drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(operator_with_concurrency_limit) <=
                      OPENDAL_MBT_API_V1_FIELD_END(operator_delete_many),
                  "v1.6 batch append order drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(operator_delete_many) <=
                      OPENDAL_MBT_API_V1_FIELD_END(operator_copier) &&
                      OPENDAL_MBT_API_V1_FIELD_END(operator_copier) <=
                          OPENDAL_MBT_API_V1_FIELD_END(copier_next) &&
                      OPENDAL_MBT_API_V1_FIELD_END(copier_next) <=
                          OPENDAL_MBT_API_V1_FIELD_END(copier_finish) &&
                      OPENDAL_MBT_API_V1_FIELD_END(copier_finish) <=
                          OPENDAL_MBT_API_V1_FIELD_END(copier_abort) &&
                      OPENDAL_MBT_API_V1_FIELD_END(copier_abort) <=
                          OPENDAL_MBT_API_V1_FIELD_END(copier_free),
                  "v1.6 Copier append order drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(copier_free) <=
                      OPENDAL_MBT_API_V1_FIELD_END(async_operator_read_start) &&
                      OPENDAL_MBT_API_V1_FIELD_END(async_operator_read_start) <=
                          OPENDAL_MBT_API_V1_FIELD_END(async_task_free),
                  "v1.7 async append order drifted");
ABI_STATIC_ASSERT(
    OPENDAL_MBT_API_V1_FIELD_END(async_task_free) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_operator_check_start) &&
        OPENDAL_MBT_API_V1_FIELD_END(async_operator_check_start) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_operator_exists_start) &&
        OPENDAL_MBT_API_V1_FIELD_END(async_operator_exists_start) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_operator_stat_start) &&
        OPENDAL_MBT_API_V1_FIELD_END(async_operator_stat_start) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_operator_write_start) &&
        OPENDAL_MBT_API_V1_FIELD_END(async_operator_write_start) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_operator_create_dir_start) &&
        OPENDAL_MBT_API_V1_FIELD_END(async_operator_create_dir_start) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_operator_delete_start) &&
        OPENDAL_MBT_API_V1_FIELD_END(async_operator_delete_start) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_operator_list_start) &&
        OPENDAL_MBT_API_V1_FIELD_END(async_operator_list_start) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_operator_copy_start) &&
        OPENDAL_MBT_API_V1_FIELD_END(async_operator_copy_start) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_operator_rename_start) &&
        OPENDAL_MBT_API_V1_FIELD_END(async_operator_rename_start) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_task_take_bool) &&
        OPENDAL_MBT_API_V1_FIELD_END(async_task_take_bool) <=
            OPENDAL_MBT_API_V1_FIELD_END(async_task_take_lister),
    "v1.8 core async append order drifted");
ABI_STATIC_ASSERT((OPENDAL_MBT_FEATURE_BATCH_DELETE &
                   OPENDAL_MBT_FEATURE_COPIER) == 0,
                  "v1.6 feature bits must be independent");
ABI_STATIC_ASSERT(
    (OPENDAL_MBT_FEATURE_ASYNC &
     (OPENDAL_MBT_FEATURE_CONCURRENCY_LIMIT |
      OPENDAL_MBT_FEATURE_BATCH_DELETE | OPENDAL_MBT_FEATURE_COPIER)) == 0,
    "v1.7 async feature bit must be independent");
ABI_STATIC_ASSERT(
    (OPENDAL_MBT_FEATURE_ASYNC_CORE &
     (OPENDAL_MBT_FEATURE_ASYNC | OPENDAL_MBT_FEATURE_BATCH_DELETE |
      OPENDAL_MBT_FEATURE_COPIER)) == 0,
    "v1.8 core async feature bit must be independent");
ABI_STATIC_ASSERT(OPENDAL_MBT_READ_OPTIONS_V1_MIN_SIZE <=
                      sizeof(opendal_mbt_read_options_v1_t),
                  "read options v1.0 prefix drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_READER_OPTIONS_V1_MIN_SIZE <=
                      sizeof(opendal_mbt_reader_options_v1_t),
                  "reader options v1.0 prefix drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_READ_STREAM_OPTIONS_V1_MIN_SIZE <=
                      sizeof(opendal_mbt_read_stream_options_v1_t),
                  "read stream options v1.1 prefix drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_WRITE_OPTIONS_V1_MIN_SIZE <=
                      sizeof(opendal_mbt_write_options_v1_t),
                  "write options v1.0 prefix drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_STAT_OPTIONS_V1_MIN_SIZE <=
                      sizeof(opendal_mbt_stat_options_v1_t),
                  "stat options v1.0 prefix drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_LIST_OPTIONS_V1_MIN_SIZE <=
                      sizeof(opendal_mbt_list_options_v1_t),
                  "list options v1.0 prefix drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_DELETE_OPTIONS_V1_MIN_SIZE <=
                      sizeof(opendal_mbt_delete_options_v1_t),
                  "delete options v1.0 prefix drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_S3_OPTIONS_V1_MIN_SIZE ==
                      sizeof(opendal_mbt_s3_options_v1_t),
                  "S3 options v1.2 prefix drifted");

static int compile_contract(void) {
  opendal_mbt_api_v1_t api;
  opendal_mbt_status_t(OPENDAL_MBT_CALL *bootstrap)(void *) =
      opendal_mbt_get_api;

  memset(&api, 0, sizeof(api));
  api.struct_size = OPENDAL_MBT_API_V1_SIZE;
  api.requested_major = OPENDAL_MBT_ABI_V1_MAJOR;

  return bootstrap(&api) == OPENDAL_MBT_STATUS_OK ? 0 : 1;
}

int main(void) { return compile_contract(); }
