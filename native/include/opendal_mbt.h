/*
 * Copyright 2026 OpenDAL MoonBit Binding contributors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#ifndef OPENDAL_MBT_H
#define OPENDAL_MBT_H

#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32)
#define OPENDAL_MBT_CALL __cdecl
#if defined(OPENDAL_MBT_SHARED)
#if defined(OPENDAL_MBT_BUILD)
#define OPENDAL_MBT_API __declspec(dllexport)
#else
#define OPENDAL_MBT_API __declspec(dllimport)
#endif
#else
#define OPENDAL_MBT_API
#endif
#elif defined(__GNUC__) || defined(__clang__)
#define OPENDAL_MBT_CALL
#define OPENDAL_MBT_API __attribute__((visibility("default")))
#else
#define OPENDAL_MBT_CALL
#define OPENDAL_MBT_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define OPENDAL_MBT_ABI_V1_MAJOR UINT32_C(1)
#define OPENDAL_MBT_ABI_V1_MINOR UINT32_C(7)
#define OPENDAL_MBT_ABI_V1_PATCH UINT32_C(0)
#define OPENDAL_MBT_STRUCT_VERSION_V1 UINT32_C(1)
#define OPENDAL_MBT_EXTENSIBLE_CODE_MAX UINT32_C(0x7fffffff)

#define OPENDAL_MBT_FIELD_END(type, field)                                   \
  ((uint32_t)(offsetof(type, field) + sizeof(((type *)0)->field)))

/*
 * Pointer contract: every pointer is required and non-NULL unless explicitly
 * optional here. Exceptions are options, out_error, config when config_len is
 * zero, batch paths when paths_len is zero, bytes_view.data when len is zero,
 * and buffer_copy.destination for a zero-capacity sizing query. Every *_free,
 * lister_close, reader_close, and read_stream_close is a no-op on NULL.
 * Non-NULL pointers must be correctly aligned, live for the call, and cover
 * the claimed readable/writable region within one allocated object; every
 * region's byte extent must fit in Rust isize. Detectable NULL, size, overflow,
 * and alignment errors return ABI_MISMATCH. Dangling, forged, out-of-bounds,
 * or stale non-NULL pointers and handles remain caller UB.
 */

/* Transport status. These values are not OpenDAL error kinds. */
typedef uint32_t opendal_mbt_status_t;
#define OPENDAL_MBT_STATUS_OK UINT32_C(0)
#define OPENDAL_MBT_STATUS_END UINT32_C(1)
#define OPENDAL_MBT_STATUS_ERROR UINT32_C(2)
#define OPENDAL_MBT_STATUS_BUFFER_TOO_SMALL UINT32_C(3)
#define OPENDAL_MBT_STATUS_ABI_MISMATCH UINT32_C(4)
#define OPENDAL_MBT_STATUS_PANIC UINT32_C(5)

/* ABI booleans are always exactly 0 or 1. */
typedef uint32_t opendal_mbt_bool_t;
#define OPENDAL_MBT_FALSE UINT32_C(0)
#define OPENDAL_MBT_TRUE UINT32_C(1)

/* Stable binding-owned error kind codes. Never cast an OpenDAL Rust enum. */
typedef uint32_t opendal_mbt_error_kind_t;
#define OPENDAL_MBT_ERROR_UNEXPECTED UINT32_C(1)
#define OPENDAL_MBT_ERROR_UNSUPPORTED UINT32_C(2)
#define OPENDAL_MBT_ERROR_CONFIG_INVALID UINT32_C(3)
#define OPENDAL_MBT_ERROR_NOT_FOUND UINT32_C(4)
#define OPENDAL_MBT_ERROR_PERMISSION_DENIED UINT32_C(5)
#define OPENDAL_MBT_ERROR_IS_A_DIRECTORY UINT32_C(6)
#define OPENDAL_MBT_ERROR_NOT_A_DIRECTORY UINT32_C(7)
#define OPENDAL_MBT_ERROR_ALREADY_EXISTS UINT32_C(8)
#define OPENDAL_MBT_ERROR_RATE_LIMITED UINT32_C(9)
#define OPENDAL_MBT_ERROR_IS_SAME_FILE UINT32_C(10)
#define OPENDAL_MBT_ERROR_CONDITION_NOT_MATCH UINT32_C(11)
#define OPENDAL_MBT_ERROR_RANGE_NOT_SATISFIED UINT32_C(12)
#define OPENDAL_MBT_ERROR_INVALID_ARGUMENT UINT32_C(0x1001)
#define OPENDAL_MBT_ERROR_RESOURCE_CLOSED UINT32_C(0x1002)
#define OPENDAL_MBT_ERROR_BUFFER_TOO_LARGE UINT32_C(0x1003)
#define OPENDAL_MBT_ERROR_ABI_MISMATCH UINT32_C(0x1004)

typedef uint32_t opendal_mbt_error_status_t;
#define OPENDAL_MBT_ERROR_STATUS_PERMANENT UINT32_C(1)
#define OPENDAL_MBT_ERROR_STATUS_TEMPORARY UINT32_C(2)
#define OPENDAL_MBT_ERROR_STATUS_PERSISTENT UINT32_C(3)

typedef uint32_t opendal_mbt_entry_mode_t;
#define OPENDAL_MBT_ENTRY_MODE_UNKNOWN UINT32_C(0)
#define OPENDAL_MBT_ENTRY_MODE_FILE UINT32_C(1)
#define OPENDAL_MBT_ENTRY_MODE_DIRECTORY UINT32_C(2)

typedef uint32_t opendal_mbt_range_kind_t;
#define OPENDAL_MBT_RANGE_FULL UINT32_C(0)
#define OPENDAL_MBT_RANGE_FROM UINT32_C(1)
#define OPENDAL_MBT_RANGE_OFFSET_LENGTH UINT32_C(2)
#define OPENDAL_MBT_RANGE_SUFFIX UINT32_C(3)

/*
 * Optional function groups in the v1 table. Every non-BASE feature requires
 * and implies BASE; callers needing a non-BASE group validate both bits.
 */
#define OPENDAL_MBT_FEATURE_BASE (UINT64_C(1) << 0)
#define OPENDAL_MBT_FEATURE_WHOLE_OBJECT (UINT64_C(1) << 1)
#define OPENDAL_MBT_FEATURE_LISTING (UINT64_C(1) << 2)
#define OPENDAL_MBT_FEATURE_RANDOM_READER (UINT64_C(1) << 3)
#define OPENDAL_MBT_FEATURE_CHUNKED_WRITER (UINT64_C(1) << 4)
#define OPENDAL_MBT_FEATURE_READ_STREAM (UINT64_C(1) << 5)
#define OPENDAL_MBT_FEATURE_WRITER_ABORT (UINT64_C(1) << 6)
#define OPENDAL_MBT_FEATURE_S3 (UINT64_C(1) << 7)
#define OPENDAL_MBT_FEATURE_PRESIGN (UINT64_C(1) << 8)
#define OPENDAL_MBT_FEATURE_LAYERS (UINT64_C(1) << 9)
#define OPENDAL_MBT_FEATURE_CONCURRENCY_LIMIT (UINT64_C(1) << 10)
#define OPENDAL_MBT_FEATURE_BATCH_DELETE (UINT64_C(1) << 11)
#define OPENDAL_MBT_FEATURE_COPIER (UINT64_C(1) << 12)
#define OPENDAL_MBT_FEATURE_ASYNC (UINT64_C(1) << 13)

/* Capability bit positions. */
#define OPENDAL_MBT_CAP_STAT (UINT64_C(1) << 0)
#define OPENDAL_MBT_CAP_READ (UINT64_C(1) << 1)
#define OPENDAL_MBT_CAP_WRITE (UINT64_C(1) << 2)
#define OPENDAL_MBT_CAP_CREATE_DIR (UINT64_C(1) << 3)
#define OPENDAL_MBT_CAP_DELETE (UINT64_C(1) << 4)
#define OPENDAL_MBT_CAP_LIST (UINT64_C(1) << 5)
#define OPENDAL_MBT_CAP_COPY (UINT64_C(1) << 6)
#define OPENDAL_MBT_CAP_RENAME (UINT64_C(1) << 7)
#define OPENDAL_MBT_CAP_READ_SUFFIX (UINT64_C(1) << 8)
#define OPENDAL_MBT_CAP_WRITE_APPEND (UINT64_C(1) << 9)
#define OPENDAL_MBT_CAP_LIST_LIMIT (UINT64_C(1) << 10)
#define OPENDAL_MBT_CAP_LIST_START_AFTER (UINT64_C(1) << 11)
#define OPENDAL_MBT_CAP_LIST_RECURSIVE (UINT64_C(1) << 12)
#define OPENDAL_MBT_CAP_PRESIGN_STAT (UINT64_C(1) << 13)
#define OPENDAL_MBT_CAP_PRESIGN_READ (UINT64_C(1) << 14)
#define OPENDAL_MBT_CAP_PRESIGN_WRITE (UINT64_C(1) << 15)

/*
 * Frozen by-value leaf layouts for ABI major v1. A byte view is borrowed for
 * one call; textual uses are strict UTF-8, not C strings.
 */
typedef struct opendal_mbt_bytes_view_v1 {
  const uint8_t *data;
  uint64_t len;
} opendal_mbt_bytes_view_v1_t;

typedef struct opendal_mbt_kv_v1 {
  opendal_mbt_bytes_view_v1_t key;
  opendal_mbt_bytes_view_v1_t value;
} opendal_mbt_kv_v1_t;

/* Frozen by-value layout for ABI major v1; fields are never appended. */
typedef struct opendal_mbt_byte_range_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  opendal_mbt_range_kind_t kind;
  uint32_t reserved0;
  uint64_t offset;
  uint64_t length;
} opendal_mbt_byte_range_v1_t;

/*
 * For every input option below, callers zero their storage, set struct_size
 * and struct_version, and use only known flags/presence bits. Within ABI major
 * v1, fields may be appended only to the end of the outer option struct; the
 * embedded leaf layouts above never grow.
 */

/* Read options presence bits. */
#define OPENDAL_MBT_READ_VERSION_PRESENT (UINT64_C(1) << 0)
#define OPENDAL_MBT_READ_IF_MATCH_PRESENT (UINT64_C(1) << 1)
#define OPENDAL_MBT_READ_IF_NONE_MATCH_PRESENT (UINT64_C(1) << 2)

typedef struct opendal_mbt_read_options_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t present_bits;
  opendal_mbt_byte_range_v1_t range;
  opendal_mbt_bytes_view_v1_t version;
  opendal_mbt_bytes_view_v1_t if_match;
  opendal_mbt_bytes_view_v1_t if_none_match;
} opendal_mbt_read_options_v1_t;
#define OPENDAL_MBT_READ_OPTIONS_V1_MIN_SIZE                                 \
  OPENDAL_MBT_FIELD_END(opendal_mbt_read_options_v1_t, if_none_match)

/* Reader options presence bits. */
#define OPENDAL_MBT_READER_VERSION_PRESENT (UINT64_C(1) << 0)
#define OPENDAL_MBT_READER_IF_MATCH_PRESENT (UINT64_C(1) << 1)
#define OPENDAL_MBT_READER_IF_NONE_MATCH_PRESENT (UINT64_C(1) << 2)

typedef struct opendal_mbt_reader_options_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t present_bits;
  opendal_mbt_bytes_view_v1_t version;
  opendal_mbt_bytes_view_v1_t if_match;
  opendal_mbt_bytes_view_v1_t if_none_match;
} opendal_mbt_reader_options_v1_t;
#define OPENDAL_MBT_READER_OPTIONS_V1_MIN_SIZE                               \
  OPENDAL_MBT_FIELD_END(opendal_mbt_reader_options_v1_t, if_none_match)

/* Read stream options presence bits. */
#define OPENDAL_MBT_READ_STREAM_VERSION_PRESENT (UINT64_C(1) << 0)
#define OPENDAL_MBT_READ_STREAM_IF_MATCH_PRESENT (UINT64_C(1) << 1)
#define OPENDAL_MBT_READ_STREAM_IF_NONE_MATCH_PRESENT (UINT64_C(1) << 2)

typedef struct opendal_mbt_read_stream_options_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t present_bits;
  opendal_mbt_byte_range_v1_t range;
  /* Required and nonzero. Each successful next result is at most this size. */
  uint64_t chunk_size;
  opendal_mbt_bytes_view_v1_t version;
  opendal_mbt_bytes_view_v1_t if_match;
  opendal_mbt_bytes_view_v1_t if_none_match;
} opendal_mbt_read_stream_options_v1_t;
#define OPENDAL_MBT_READ_STREAM_OPTIONS_V1_MIN_SIZE                           \
  OPENDAL_MBT_FIELD_END(opendal_mbt_read_stream_options_v1_t, if_none_match)

/* Write options flags and presence bits. */
#define OPENDAL_MBT_WRITE_APPEND (UINT64_C(1) << 0)
#define OPENDAL_MBT_WRITE_CONTENT_TYPE_PRESENT (UINT64_C(1) << 0)
#define OPENDAL_MBT_WRITE_CONTENT_DISPOSITION_PRESENT (UINT64_C(1) << 1)
#define OPENDAL_MBT_WRITE_CONTENT_ENCODING_PRESENT (UINT64_C(1) << 2)
#define OPENDAL_MBT_WRITE_CACHE_CONTROL_PRESENT (UINT64_C(1) << 3)
#define OPENDAL_MBT_WRITE_IF_MATCH_PRESENT (UINT64_C(1) << 4)
#define OPENDAL_MBT_WRITE_IF_NONE_MATCH_PRESENT (UINT64_C(1) << 5)

typedef struct opendal_mbt_write_options_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t present_bits;
  uint64_t flags;
  opendal_mbt_bytes_view_v1_t content_type;
  opendal_mbt_bytes_view_v1_t content_disposition;
  opendal_mbt_bytes_view_v1_t content_encoding;
  opendal_mbt_bytes_view_v1_t cache_control;
  opendal_mbt_bytes_view_v1_t if_match;
  opendal_mbt_bytes_view_v1_t if_none_match;
} opendal_mbt_write_options_v1_t;
#define OPENDAL_MBT_WRITE_OPTIONS_V1_MIN_SIZE                                \
  OPENDAL_MBT_FIELD_END(opendal_mbt_write_options_v1_t, if_none_match)

/* Stat options presence bits. */
#define OPENDAL_MBT_STAT_VERSION_PRESENT (UINT64_C(1) << 0)
#define OPENDAL_MBT_STAT_IF_MATCH_PRESENT (UINT64_C(1) << 1)
#define OPENDAL_MBT_STAT_IF_NONE_MATCH_PRESENT (UINT64_C(1) << 2)

typedef struct opendal_mbt_stat_options_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t present_bits;
  opendal_mbt_bytes_view_v1_t version;
  opendal_mbt_bytes_view_v1_t if_match;
  opendal_mbt_bytes_view_v1_t if_none_match;
} opendal_mbt_stat_options_v1_t;
#define OPENDAL_MBT_STAT_OPTIONS_V1_MIN_SIZE                                 \
  OPENDAL_MBT_FIELD_END(opendal_mbt_stat_options_v1_t, if_none_match)

/* List options flags and presence bits. */
#define OPENDAL_MBT_LIST_RECURSIVE (UINT64_C(1) << 0)
#define OPENDAL_MBT_LIST_LIMIT_PRESENT (UINT64_C(1) << 0)
#define OPENDAL_MBT_LIST_START_AFTER_PRESENT (UINT64_C(1) << 1)

typedef struct opendal_mbt_list_options_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t present_bits;
  uint64_t flags;
  /* Must be zero when OPENDAL_MBT_LIST_LIMIT_PRESENT is clear. */
  uint64_t limit;
  opendal_mbt_bytes_view_v1_t start_after;
} opendal_mbt_list_options_v1_t;
#define OPENDAL_MBT_LIST_OPTIONS_V1_MIN_SIZE                                 \
  OPENDAL_MBT_FIELD_END(opendal_mbt_list_options_v1_t, start_after)

/* Delete options flags and presence bits. */
#define OPENDAL_MBT_DELETE_RECURSIVE (UINT64_C(1) << 0)
#define OPENDAL_MBT_DELETE_VERSION_PRESENT (UINT64_C(1) << 0)

typedef struct opendal_mbt_delete_options_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t present_bits;
  uint64_t flags;
  opendal_mbt_bytes_view_v1_t version;
} opendal_mbt_delete_options_v1_t;
#define OPENDAL_MBT_DELETE_OPTIONS_V1_MIN_SIZE                               \
  OPENDAL_MBT_FIELD_END(opendal_mbt_delete_options_v1_t, version)

/* Typed S3 constructor authentication modes. */
typedef uint32_t opendal_mbt_s3_auth_kind_t;
#define OPENDAL_MBT_S3_AUTH_DEFAULT_CHAIN UINT32_C(0)
#define OPENDAL_MBT_S3_AUTH_STATIC UINT32_C(1)
#define OPENDAL_MBT_S3_AUTH_UNSIGNED UINT32_C(2)
#define OPENDAL_MBT_S3_AUTH_ASSUME_ROLE UINT32_C(3)

/* Credential sources accepted by the assume-role authentication mode. */
typedef uint32_t opendal_mbt_s3_assume_role_source_kind_t;
#define OPENDAL_MBT_S3_SOURCE_DEFAULT_CHAIN UINT32_C(0)
#define OPENDAL_MBT_S3_SOURCE_STATIC UINT32_C(1)

/* S3 options presence bits and flags. */
#define OPENDAL_MBT_S3_ROOT_PRESENT (UINT64_C(1) << 0)
#define OPENDAL_MBT_S3_ENDPOINT_PRESENT (UINT64_C(1) << 1)
#define OPENDAL_MBT_S3_SESSION_TOKEN_PRESENT (UINT64_C(1) << 2)
#define OPENDAL_MBT_S3_EXTERNAL_ID_PRESENT (UINT64_C(1) << 3)
#define OPENDAL_MBT_S3_ROLE_SESSION_NAME_PRESENT (UINT64_C(1) << 4)
#define OPENDAL_MBT_S3_ASSUME_ROLE_DURATION_PRESENT (UINT64_C(1) << 5)
#define OPENDAL_MBT_S3_VIRTUAL_HOST_STYLE (UINT64_C(1) << 0)
#define OPENDAL_MBT_S3_DISABLE_EC2_METADATA (UINT64_C(1) << 1)

/*
 * Versioned typed S3 constructor input. All text is strict UTF-8 and borrowed
 * for one call; the library copies every supplied value before returning.
 * bucket and region are required and non-empty. Optional views must be the
 * canonical {NULL, 0} value when their presence bit is clear. Credential and
 * role views not selected by auth_kind/source_kind must also be canonical.
 */
typedef struct opendal_mbt_s3_options_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t present_bits;
  uint64_t flags;
  opendal_mbt_s3_auth_kind_t auth_kind;
  opendal_mbt_s3_assume_role_source_kind_t source_kind;
  opendal_mbt_bytes_view_v1_t bucket;
  opendal_mbt_bytes_view_v1_t region;
  opendal_mbt_bytes_view_v1_t root;
  opendal_mbt_bytes_view_v1_t endpoint;
  opendal_mbt_bytes_view_v1_t access_key_id;
  opendal_mbt_bytes_view_v1_t secret_access_key;
  opendal_mbt_bytes_view_v1_t session_token;
  opendal_mbt_bytes_view_v1_t role_arn;
  opendal_mbt_bytes_view_v1_t external_id;
  opendal_mbt_bytes_view_v1_t role_session_name;
  uint32_t assume_role_duration_seconds;
  uint32_t reserved0;
} opendal_mbt_s3_options_v1_t;
#define OPENDAL_MBT_S3_OPTIONS_V1_MIN_SIZE                                   \
  OPENDAL_MBT_FIELD_END(opendal_mbt_s3_options_v1_t, reserved0)

typedef struct opendal_mbt_timestamp_v1 {
  int64_t unix_seconds;
  uint32_t nanoseconds;
  uint32_t reserved0;
} opendal_mbt_timestamp_v1_t;

typedef struct opendal_mbt_capability_v1 {
  /* words[0] contains boolean CAP_* flags; words[1] is delete_max_size or 0. */
  uint64_t words[4];
} opendal_mbt_capability_v1_t;

/*
 * All output *_view_v1 layouts below are frozen for ABI major v1. Callers
 * zero sizeof(view), then set struct_size and struct_version. Inspectors write
 * exactly the v1 prefix and never a caller tail. Future fields use a newly
 * named view type and an appended inspector function.
 */

/* Metadata view presence bits. */
#define OPENDAL_MBT_METADATA_IS_CURRENT_PRESENT (UINT64_C(1) << 0)
#define OPENDAL_MBT_METADATA_LAST_MODIFIED_PRESENT (UINT64_C(1) << 1)
#define OPENDAL_MBT_METADATA_CACHE_CONTROL_PRESENT (UINT64_C(1) << 2)
#define OPENDAL_MBT_METADATA_CONTENT_DISPOSITION_PRESENT (UINT64_C(1) << 3)
#define OPENDAL_MBT_METADATA_CONTENT_ENCODING_PRESENT (UINT64_C(1) << 4)
#define OPENDAL_MBT_METADATA_CONTENT_MD5_PRESENT (UINT64_C(1) << 5)
#define OPENDAL_MBT_METADATA_CONTENT_TYPE_PRESENT (UINT64_C(1) << 6)
#define OPENDAL_MBT_METADATA_ETAG_PRESENT (UINT64_C(1) << 7)
#define OPENDAL_MBT_METADATA_VERSION_PRESENT (UINT64_C(1) << 8)

typedef struct opendal_mbt_metadata_view_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t present_bits;
  opendal_mbt_entry_mode_t mode;
  opendal_mbt_bool_t is_current;
  opendal_mbt_bool_t is_deleted;
  uint32_t reserved0;
  uint64_t content_length;
  opendal_mbt_timestamp_v1_t last_modified;
  opendal_mbt_bytes_view_v1_t cache_control;
  opendal_mbt_bytes_view_v1_t content_disposition;
  opendal_mbt_bytes_view_v1_t content_encoding;
  opendal_mbt_bytes_view_v1_t content_md5;
  opendal_mbt_bytes_view_v1_t content_type;
  opendal_mbt_bytes_view_v1_t etag;
  opendal_mbt_bytes_view_v1_t version;
} opendal_mbt_metadata_view_v1_t;

typedef struct opendal_mbt_entry_view_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t reserved0;
  opendal_mbt_bytes_view_v1_t path;
  opendal_mbt_bytes_view_v1_t name;
} opendal_mbt_entry_view_v1_t;

typedef struct opendal_mbt_operator_info_view_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t reserved0;
  opendal_mbt_bytes_view_v1_t scheme;
  opendal_mbt_bytes_view_v1_t root;
  opendal_mbt_bytes_view_v1_t name;
  opendal_mbt_capability_v1_t capability;
} opendal_mbt_operator_info_view_v1_t;

typedef struct opendal_mbt_error_view_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  opendal_mbt_error_kind_t kind;
  opendal_mbt_error_status_t status;
  opendal_mbt_bytes_view_v1_t kind_name;
  opendal_mbt_bytes_view_v1_t message;
} opendal_mbt_error_view_v1_t;

typedef struct opendal_mbt_library_info_view_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t reserved0;
  opendal_mbt_bytes_view_v1_t binding_version;
  opendal_mbt_bytes_view_v1_t opendal_version;
  opendal_mbt_bytes_view_v1_t service_profile;
} opendal_mbt_library_info_view_v1_t;

typedef struct opendal_mbt_presigned_request_view_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t reserved0;
  opendal_mbt_bytes_view_v1_t method;
  opendal_mbt_bytes_view_v1_t uri;
  uint64_t header_count;
} opendal_mbt_presigned_request_view_v1_t;

typedef struct opendal_mbt_presigned_header_view_v1 {
  uint32_t struct_size;
  uint32_t struct_version;
  uint64_t reserved0;
  opendal_mbt_bytes_view_v1_t name;
  opendal_mbt_bytes_view_v1_t value;
} opendal_mbt_presigned_header_view_v1_t;

/* Rust-owned opaque handles. */
typedef struct opendal_mbt_operator_v1 opendal_mbt_operator_v1_t;
typedef struct opendal_mbt_lister_v1 opendal_mbt_lister_v1_t;
typedef struct opendal_mbt_reader_v1 opendal_mbt_reader_v1_t;
typedef struct opendal_mbt_writer_v1 opendal_mbt_writer_v1_t;
typedef struct opendal_mbt_read_stream_v1 opendal_mbt_read_stream_v1_t;
typedef struct opendal_mbt_copier_v1 opendal_mbt_copier_v1_t;
typedef struct opendal_mbt_buffer_v1 opendal_mbt_buffer_v1_t;
typedef struct opendal_mbt_error_v1 opendal_mbt_error_v1_t;
typedef struct opendal_mbt_metadata_v1 opendal_mbt_metadata_v1_t;
typedef struct opendal_mbt_entry_v1 opendal_mbt_entry_v1_t;
typedef struct opendal_mbt_operator_info_v1 opendal_mbt_operator_info_v1_t;
typedef struct opendal_mbt_presigned_request_v1
    opendal_mbt_presigned_request_v1_t;
typedef struct opendal_mbt_async_task_v1 opendal_mbt_async_task_v1_t;
typedef struct opendal_mbt_async_read_stream_v1
    opendal_mbt_async_read_stream_v1_t;
typedef struct opendal_mbt_async_writer_v1 opendal_mbt_async_writer_v1_t;

/*
 * Append-only v1 function table. All table function pointers use the C calling
 * convention. A released full v1 binding provides every field; feature bits
 * permit staged development and future reduced-profile builds.
 */
typedef struct opendal_mbt_api_v1 {
  /* Caller input. The caller zeroes the table, then sets these two fields. */
  uint32_t struct_size;
  uint32_t requested_major;

  /* Library output. */
  uint32_t library_struct_size;
  uint32_t library_minor;
  uint32_t library_patch;
  uint32_t reserved0;
  uint64_t feature_bits;
  uint64_t max_output_bytes;

  /* OPENDAL_MBT_FEATURE_BASE: library_info through operator_free. */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *library_info)(
      opendal_mbt_library_info_view_v1_t *out_info);

  opendal_mbt_status_t(OPENDAL_MBT_CALL *error_view)(
      const opendal_mbt_error_v1_t *error,
      opendal_mbt_error_view_v1_t *out_view);
  void(OPENDAL_MBT_CALL *error_free)(opendal_mbt_error_v1_t *error);

  opendal_mbt_status_t(OPENDAL_MBT_CALL *buffer_len)(
      const opendal_mbt_buffer_v1_t *buffer, uint64_t *out_len);
  /*
   * out_required is required. NULL+zero destination is a sizing query: OK for
   * empty, BUFFER_TOO_SMALL for non-empty. BUFFER_TOO_SMALL writes only the
   * exact required length; NULL with nonzero capacity is ABI_MISMATCH. Every
   * non-OK status leaves destination untouched; OK writes only the required
   * prefix and never the remaining capacity.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *buffer_copy)(
      const opendal_mbt_buffer_v1_t *buffer, uint8_t *destination,
      uint64_t capacity, uint64_t *out_required);
  void(OPENDAL_MBT_CALL *buffer_free)(opendal_mbt_buffer_v1_t *buffer);

  opendal_mbt_status_t(OPENDAL_MBT_CALL *metadata_view)(
      const opendal_mbt_metadata_v1_t *metadata,
      opendal_mbt_metadata_view_v1_t *out_view);
  void(OPENDAL_MBT_CALL *metadata_free)(opendal_mbt_metadata_v1_t *metadata);

  opendal_mbt_status_t(OPENDAL_MBT_CALL *entry_view)(
      const opendal_mbt_entry_v1_t *entry,
      opendal_mbt_entry_view_v1_t *out_view);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *entry_metadata_view)(
      const opendal_mbt_entry_v1_t *entry,
      opendal_mbt_metadata_view_v1_t *out_view);
  void(OPENDAL_MBT_CALL *entry_free)(opendal_mbt_entry_v1_t *entry);

  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_info_view)(
      const opendal_mbt_operator_info_v1_t *info,
      opendal_mbt_operator_info_view_v1_t *out_view);
  void(OPENDAL_MBT_CALL *operator_info_free)(
      opendal_mbt_operator_info_v1_t *info);

  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_new)(
      const opendal_mbt_bytes_view_v1_t *scheme,
      const opendal_mbt_kv_v1_t *config, uint64_t config_len,
      opendal_mbt_operator_v1_t **out_operator,
      opendal_mbt_operator_info_v1_t **out_info,
      opendal_mbt_error_v1_t **out_error);
  void(OPENDAL_MBT_CALL *operator_free)(opendal_mbt_operator_v1_t *operator_);

  /* OPENDAL_MBT_FEATURE_WHOLE_OBJECT: operator_check through operator_rename. */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_check)(
      opendal_mbt_operator_v1_t *operator_,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_exists)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      opendal_mbt_bool_t *out_exists,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_stat)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_stat_options_v1_t *options,
      opendal_mbt_metadata_v1_t **out_metadata,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_read)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_read_options_v1_t *options,
      uint64_t max_output_len, opendal_mbt_buffer_v1_t **out_buffer,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_write)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_bytes_view_v1_t *data,
      const opendal_mbt_write_options_v1_t *options,
      opendal_mbt_metadata_v1_t **out_metadata,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_create_dir)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_delete)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_delete_options_v1_t *options,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_copy)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *source,
      const opendal_mbt_bytes_view_v1_t *destination,
      opendal_mbt_metadata_v1_t **out_metadata,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_rename)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *source,
      const opendal_mbt_bytes_view_v1_t *destination,
      opendal_mbt_error_v1_t **out_error);

  /* OPENDAL_MBT_FEATURE_LISTING: operator_lister through lister_free. */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_lister)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_list_options_v1_t *options,
      opendal_mbt_lister_v1_t **out_lister,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *lister_next)(
      opendal_mbt_lister_v1_t *lister,
      opendal_mbt_entry_v1_t **out_entry,
      opendal_mbt_error_v1_t **out_error);
  /* NULL is a no-op. */
  void(OPENDAL_MBT_CALL *lister_close)(opendal_mbt_lister_v1_t *lister);
  void(OPENDAL_MBT_CALL *lister_free)(opendal_mbt_lister_v1_t *lister);

  /* OPENDAL_MBT_FEATURE_RANDOM_READER: operator_reader through reader_free. */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_reader)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_reader_options_v1_t *options,
      opendal_mbt_reader_v1_t **out_reader,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *reader_read)(
      opendal_mbt_reader_v1_t *reader,
      const opendal_mbt_byte_range_v1_t *range,
      uint64_t max_output_len, opendal_mbt_buffer_v1_t **out_buffer,
      opendal_mbt_error_v1_t **out_error);
  /* NULL is a no-op. */
  void(OPENDAL_MBT_CALL *reader_close)(opendal_mbt_reader_v1_t *reader);
  void(OPENDAL_MBT_CALL *reader_free)(opendal_mbt_reader_v1_t *reader);

  /* OPENDAL_MBT_FEATURE_CHUNKED_WRITER: operator_writer through writer_free. */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_writer)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_write_options_v1_t *options,
      opendal_mbt_writer_v1_t **out_writer,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *writer_write)(
      opendal_mbt_writer_v1_t *writer,
      const opendal_mbt_bytes_view_v1_t *data,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *writer_close)(
      opendal_mbt_writer_v1_t *writer,
      opendal_mbt_metadata_v1_t **out_metadata,
      opendal_mbt_error_v1_t **out_error);
  void(OPENDAL_MBT_CALL *writer_free)(opendal_mbt_writer_v1_t *writer);

  /*
   * OPENDAL_MBT_FEATURE_READ_STREAM: operator_read_stream through
   * read_stream_free. Appended in ABI v1.1.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_read_stream)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_read_stream_options_v1_t *options,
      opendal_mbt_read_stream_v1_t **out_stream,
      opendal_mbt_error_v1_t **out_error);
  /*
   * OK returns one non-NULL buffer. END returns NULL buffer and no error.
   * ERROR returns NULL buffer and an optional owned error. Stream errors are
   * terminal; END is stable. max_output_len must cover the configured chunk.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *read_stream_next)(
      opendal_mbt_read_stream_v1_t *stream, uint64_t max_output_len,
      opendal_mbt_buffer_v1_t **out_buffer,
      opendal_mbt_error_v1_t **out_error);
  /* NULL is a no-op; close is idempotent and never performs I/O. */
  void(OPENDAL_MBT_CALL *read_stream_close)(
      opendal_mbt_read_stream_v1_t *stream);
  void(OPENDAL_MBT_CALL *read_stream_free)(
      opendal_mbt_read_stream_v1_t *stream);

  /*
   * OPENDAL_MBT_FEATURE_WRITER_ABORT: appended in ABI v1.1 and dependent on
   * BASE plus CHUNKED_WRITER. A successful repeated abort is idempotent.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *writer_abort)(
      opendal_mbt_writer_v1_t *writer,
      opendal_mbt_error_v1_t **out_error);

  /*
   * OPENDAL_MBT_FEATURE_S3: appended in ABI v1.2 and dependent on BASE.
   * Construction performs no object-store I/O and returns ordinary owned
   * Operator and OperatorInfo handles.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_s3)(
      const opendal_mbt_s3_options_v1_t *options,
      opendal_mbt_operator_v1_t **out_operator,
      opendal_mbt_operator_info_v1_t **out_info,
      opendal_mbt_error_v1_t **out_error);

  /*
   * OPENDAL_MBT_FEATURE_PRESIGN: appended in ABI v1.3. Request and header
   * views borrow from the immutable request handle until it is freed. Header
   * order and repeated names are preserved; header values are arbitrary bytes.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_presign_read)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_read_options_v1_t *options, uint64_t expires_in_seconds,
      opendal_mbt_presigned_request_v1_t **out_request,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_presign_write)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_write_options_v1_t *options,
      uint64_t expires_in_seconds,
      opendal_mbt_presigned_request_v1_t **out_request,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_presign_stat)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_stat_options_v1_t *options, uint64_t expires_in_seconds,
      opendal_mbt_presigned_request_v1_t **out_request,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *presigned_request_view)(
      const opendal_mbt_presigned_request_v1_t *request,
      opendal_mbt_presigned_request_view_v1_t *out_view);
  /* Returns END when index is outside [0, header_count). */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *presigned_request_header_view)(
      const opendal_mbt_presigned_request_v1_t *request, uint64_t index,
      opendal_mbt_presigned_header_view_v1_t *out_view);
  void(OPENDAL_MBT_CALL *presigned_request_free)(
      opendal_mbt_presigned_request_v1_t *request);

  /*
   * OPENDAL_MBT_FEATURE_LAYERS: appended in ABI v1.4 and dependent on BASE.
   * Each call borrows operator_ and returns a separately owned Operator plus
   * its immutable OperatorInfo snapshot. The input Operator and every resource
   * already opened from it are unchanged.
   * Layer order is exactly call order: the newly requested layer is appended
   * outside the input Operator's existing layer stack. Timeout values and
   * delay bounds must be nonzero; min_delay_millis must not exceed
   * max_delay_millis. max_retries counts attempts after the initial request,
   * and jitter must be exactly FALSE or TRUE. Duplicate timeout/retry layers
   * and adding timeout after retry are rejected.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_with_timeout)(
      const opendal_mbt_operator_v1_t *operator_,
      uint64_t operation_timeout_millis, uint64_t io_timeout_millis,
      opendal_mbt_operator_v1_t **out_operator,
      opendal_mbt_operator_info_v1_t **out_info,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_with_retry)(
      const opendal_mbt_operator_v1_t *operator_, uint32_t max_retries,
      uint64_t min_delay_millis, uint64_t max_delay_millis,
      opendal_mbt_bool_t jitter,
      opendal_mbt_operator_v1_t **out_operator,
      opendal_mbt_operator_info_v1_t **out_info,
      opendal_mbt_error_v1_t **out_error);

  /*
   * OPENDAL_MBT_FEATURE_CONCURRENCY_LIMIT: appended in ABI v1.5 and dependent
   * on BASE. The call borrows operator_ and returns a separately owned
   * Operator plus its immutable OperatorInfo snapshot. operation_limit must
   * be nonzero and representable as a native usize. When
   * has_http_request_limit is TRUE, http_request_limit has the same rules;
   * the flag must be exactly FALSE or TRUE. When the flag is FALSE,
   * http_request_limit must be zero.
   *
   * This layer is always appended outside timeout and retry. It can be added
   * only once; timeout and retry cannot subsequently be appended outside it.
   * Operation permits for body-style Reader streams, Writers, Listers,
   * Deleters, and Copiers remain held for the body's lifetime. An optional
   * HTTP permit remains held until the response body is dropped.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_with_concurrency_limit)(
      const opendal_mbt_operator_v1_t *operator_, uint64_t operation_limit,
      opendal_mbt_bool_t has_http_request_limit,
      uint64_t http_request_limit,
      opendal_mbt_operator_v1_t **out_operator,
      opendal_mbt_operator_info_v1_t **out_info,
      opendal_mbt_error_v1_t **out_error);

  /*
   * OPENDAL_MBT_FEATURE_BATCH_DELETE: appended in ABI v1.6. Paths are copied
   * before OpenDAL work begins. Success means the high-level deleter closed;
   * an error may follow deletion of an unspecified subset. Input order and
   * duplicate identity are not observable results. The carrier plus path byte
   * extents must fit max_output_bytes; larger input returns BufferTooLarge.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_delete_many)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *paths, uint64_t paths_len,
      opendal_mbt_error_v1_t **out_error);

  /*
   * OPENDAL_MBT_FEATURE_COPIER: appended in ABI v1.6. This is OpenDAL's
   * same-Operator, one-object Copier rather than a recursive transfer engine.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *operator_copier)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *source,
      const opendal_mbt_bytes_view_v1_t *destination,
      opendal_mbt_copier_v1_t **out_copier,
      opendal_mbt_error_v1_t **out_error);
  /* OK writes one progress delta. END writes zero and is stable until finish. */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *copier_next)(
      opendal_mbt_copier_v1_t *copier, uint64_t *out_bytes,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *copier_finish)(
      opendal_mbt_copier_v1_t *copier,
      opendal_mbt_metadata_v1_t **out_metadata,
      opendal_mbt_error_v1_t **out_error);
  /* A successful repeated abort is idempotent. */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *copier_abort)(
      opendal_mbt_copier_v1_t *copier,
      opendal_mbt_error_v1_t **out_error);
  /* NULL is a no-op; free never finishes or reports a successful abort. */
  void(OPENDAL_MBT_CALL *copier_free)(opendal_mbt_copier_v1_t *copier);

  /*
   * OPENDAL_MBT_FEATURE_ASYNC: appended in ABI v1.7. This group is available
   * on the advertised macOS and Linux targets and depends on BASE.
   * `completion_fd` must be the
   * writable end of a fresh, empty pipe dedicated to this task and already
   * configured with O_NONBLOCK; a blocking or non-pipe descriptor is rejected.
   * A start attempt may duplicate and configure the descriptor even if a
   * later validation step fails. After every attempt the caller must only
   * close its original descriptor: it must not write through it, reuse it for
   * another task, or change its shared file-status flags. On success the
   * duplicate lives until the task is terminal. All other borrowed inputs are
   * copied and may be reused at once.
   * Worker completion publishes one owned result before attempting to write
   * one byte. EAGAIN is ignored as a nonblocking defensive fallback, and EPIPE
   * is contained without changing the process-wide SIGPIPE disposition.
   * Cancellation that wins first publishes the terminal task state and owns
   * that write attempt; the later worker does not write again. The byte is
   * readiness only and carries no data. Workers never call the foreign runtime
   * and never retain foreign values.
   */
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_operator_read_start)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_read_options_v1_t *options, uint64_t max_output_len,
      int32_t completion_fd, opendal_mbt_async_task_v1_t **out_task,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_operator_read_stream_start)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_read_stream_options_v1_t *options,
      int32_t completion_fd, opendal_mbt_async_task_v1_t **out_task,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_read_stream_next_start)(
      opendal_mbt_async_read_stream_v1_t *stream, uint64_t max_output_len,
      int32_t completion_fd, opendal_mbt_async_task_v1_t **out_task,
      opendal_mbt_error_v1_t **out_error);
  /* NULL is a no-op; close is idempotent, synchronous, and performs no I/O. */
  void(OPENDAL_MBT_CALL *async_read_stream_close)(
      opendal_mbt_async_read_stream_v1_t *stream);
  void(OPENDAL_MBT_CALL *async_read_stream_free)(
      opendal_mbt_async_read_stream_v1_t *stream);

  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_operator_writer_start)(
      opendal_mbt_operator_v1_t *operator_,
      const opendal_mbt_bytes_view_v1_t *path,
      const opendal_mbt_write_options_v1_t *options, int32_t completion_fd,
      opendal_mbt_async_task_v1_t **out_task,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_writer_write_start)(
      opendal_mbt_async_writer_v1_t *writer,
      const opendal_mbt_bytes_view_v1_t *data, int32_t completion_fd,
      opendal_mbt_async_task_v1_t **out_task,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_writer_finish_start)(
      opendal_mbt_async_writer_v1_t *writer, int32_t completion_fd,
      opendal_mbt_async_task_v1_t **out_task,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_writer_abort_start)(
      opendal_mbt_async_writer_v1_t *writer, int32_t completion_fd,
      opendal_mbt_async_task_v1_t **out_task,
      opendal_mbt_error_v1_t **out_error);
  void(OPENDAL_MBT_CALL *async_writer_free)(
      opendal_mbt_async_writer_v1_t *writer);

  /*
   * Cancellation and free are idempotent on NULL. A task result is taken
   * exactly once with the matching typed take. Taking too early, after cancel,
   * twice, or through the wrong typed function returns ERROR. Cancel after a
   * successful take is a no-op. Cancelling an unclaimed stream/writer result
   * makes that resource terminal even when the worker already completed.
   */
  void(OPENDAL_MBT_CALL *async_task_cancel)(
      opendal_mbt_async_task_v1_t *task);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_task_take_buffer)(
      opendal_mbt_async_task_v1_t *task,
      opendal_mbt_buffer_v1_t **out_buffer,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_task_take_metadata)(
      opendal_mbt_async_task_v1_t *task,
      opendal_mbt_metadata_v1_t **out_metadata,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_task_take_read_stream)(
      opendal_mbt_async_task_v1_t *task,
      opendal_mbt_async_read_stream_v1_t **out_stream,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_task_take_writer)(
      opendal_mbt_async_task_v1_t *task,
      opendal_mbt_async_writer_v1_t **out_writer,
      opendal_mbt_error_v1_t **out_error);
  opendal_mbt_status_t(OPENDAL_MBT_CALL *async_task_take_unit)(
      opendal_mbt_async_task_v1_t *task,
      opendal_mbt_error_v1_t **out_error);
  void(OPENDAL_MBT_CALL *async_task_free)(
      opendal_mbt_async_task_v1_t *task);
} opendal_mbt_api_v1_t;

/* End offset of one complete table field; never read a partially covered one. */
#define OPENDAL_MBT_API_V1_FIELD_END(field)                                  \
  OPENDAL_MBT_FIELD_END(opendal_mbt_api_v1_t, field)
/* Permanent caller-input prefix, through requested_major. */
#define OPENDAL_MBT_API_V1_INPUT_SIZE                                        \
  OPENDAL_MBT_API_V1_FIELD_END(requested_major)
/* Permanent v1 bootstrap-output prefix, through max_output_bytes. */
#define OPENDAL_MBT_API_V1_PREFIX_SIZE                                        \
  OPENDAL_MBT_API_V1_FIELD_END(max_output_bytes)
#define OPENDAL_MBT_API_V1_SIZE ((uint32_t)sizeof(opendal_mbt_api_v1_t))

/*
 * The only exported bootstrap symbol. The caller passes a zeroed v1 table,
 * sets struct_size (at least OPENDAL_MBT_API_V1_PREFIX_SIZE) and
 * requested_major, and checks the returned status before using any function
 * pointer. inout_api is required, non-NULL, aligned, and live/readable/writable
 * for at least OPENDAL_MBT_API_V1_INPUT_SIZE and then for struct_size bytes.
 * The library writes a table field only if that entire field is covered by
 * both caller and library sizes; it never treats short caller storage as a
 * complete opendal_mbt_api_v1_t. Panics return PANIC without partial fields.
 */
OPENDAL_MBT_API opendal_mbt_status_t OPENDAL_MBT_CALL
opendal_mbt_get_api(void *inout_api);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* OPENDAL_MBT_H */
