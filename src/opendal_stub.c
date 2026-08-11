#include <moonbit.h>

#include "../native/include/opendal_mbt.h"

#include <limits.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct moonbit_opendal_operator {
  opendal_mbt_operator_v1_t *operator_;
  opendal_mbt_operator_info_v1_t *info;
} moonbit_opendal_operator_t;

typedef struct moonbit_opendal_capability {
  uint64_t words[4];
} moonbit_opendal_capability_t;

typedef struct moonbit_opendal_metadata {
  opendal_mbt_metadata_v1_t *metadata;
  opendal_mbt_entry_v1_t *entry;
} moonbit_opendal_metadata_t;

typedef struct moonbit_opendal_lister {
  opendal_mbt_lister_v1_t *lister;
} moonbit_opendal_lister_t;

typedef struct moonbit_opendal_reader {
  opendal_mbt_reader_v1_t *reader;
} moonbit_opendal_reader_t;

typedef struct moonbit_opendal_read_stream {
  opendal_mbt_read_stream_v1_t *stream;
} moonbit_opendal_read_stream_t;

typedef struct moonbit_opendal_writer {
  opendal_mbt_writer_v1_t *writer;
} moonbit_opendal_writer_t;

typedef struct moonbit_opendal_copier {
  opendal_mbt_copier_v1_t *copier;
} moonbit_opendal_copier_t;

typedef enum moonbit_opendal_async_result_kind {
  MOONBIT_OPENDAL_ASYNC_BUFFER = 1,
  MOONBIT_OPENDAL_ASYNC_METADATA = 2,
  MOONBIT_OPENDAL_ASYNC_READ_STREAM = 3,
  MOONBIT_OPENDAL_ASYNC_WRITER = 4,
  MOONBIT_OPENDAL_ASYNC_UNIT = 5,
} moonbit_opendal_async_result_kind_t;

typedef struct moonbit_opendal_async_task {
  opendal_mbt_async_task_v1_t *task;
  moonbit_opendal_async_result_kind_t kind;
} moonbit_opendal_async_task_t;

typedef struct moonbit_opendal_async_read_stream {
  opendal_mbt_async_read_stream_v1_t *stream;
} moonbit_opendal_async_read_stream_t;

typedef struct moonbit_opendal_async_writer {
  opendal_mbt_async_writer_v1_t *writer;
} moonbit_opendal_async_writer_t;

typedef struct moonbit_opendal_entry {
  opendal_mbt_entry_v1_t *entry;
} moonbit_opendal_entry_t;

typedef struct moonbit_opendal_presigned_request {
  opendal_mbt_presigned_request_v1_t *request;
} moonbit_opendal_presigned_request_t;

typedef struct moonbit_opendal_result {
  opendal_mbt_status_t status;
  opendal_mbt_bool_t bool_value;
  bool has_bool;
  uint64_t uint64_value;
  bool has_uint64;
  opendal_mbt_error_kind_t local_kind;
  opendal_mbt_error_status_t local_error_status;
  const char *local_kind_name;
  const char *local_message;
  opendal_mbt_error_v1_t *error;
  opendal_mbt_operator_v1_t *operator_;
  opendal_mbt_operator_info_v1_t *info;
  opendal_mbt_buffer_v1_t *buffer;
  opendal_mbt_metadata_v1_t *metadata;
  opendal_mbt_lister_v1_t *lister;
  opendal_mbt_reader_v1_t *reader;
  opendal_mbt_read_stream_v1_t *read_stream;
  opendal_mbt_writer_v1_t *writer;
  opendal_mbt_copier_v1_t *copier;
  opendal_mbt_entry_v1_t *entry;
  opendal_mbt_presigned_request_v1_t *presigned_request;
  opendal_mbt_async_task_v1_t *async_task;
  moonbit_opendal_async_result_kind_t async_kind;
  opendal_mbt_async_read_stream_v1_t *async_read_stream;
  opendal_mbt_async_writer_v1_t *async_writer;
} moonbit_opendal_result_t;

typedef struct owned_utf8 {
  uint8_t *data;
  uint64_t len;
} owned_utf8_t;

typedef enum utf16_result {
  UTF16_OK = 0,
  UTF16_INVALID = 1,
  UTF16_NO_MEMORY = 2,
} utf16_result_t;

#define API_HAS(api, field)                                                   \
  ((api)->struct_size >= OPENDAL_MBT_API_V1_FIELD_END(field) &&              \
   (api)->library_struct_size >= OPENDAL_MBT_API_V1_FIELD_END(field) &&      \
   (api)->field != NULL)

static opendal_mbt_status_t load_api(opendal_mbt_api_v1_t *api,
                                     bool require_whole_object) {
  opendal_mbt_status_t status;
  memset(api, 0, sizeof(*api));
  api->struct_size = OPENDAL_MBT_API_V1_SIZE;
  api->requested_major = OPENDAL_MBT_ABI_V1_MAJOR;
  status = opendal_mbt_get_api(api);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if ((api->feature_bits & OPENDAL_MBT_FEATURE_BASE) == 0 ||
      !API_HAS(api, error_view) || !API_HAS(api, error_free) ||
      !API_HAS(api, buffer_len) || !API_HAS(api, buffer_copy) ||
      !API_HAS(api, buffer_free) || !API_HAS(api, metadata_view) ||
      !API_HAS(api, metadata_free) || !API_HAS(api, entry_view) ||
      !API_HAS(api, entry_metadata_view) || !API_HAS(api, entry_free) ||
      !API_HAS(api, operator_info_view) || !API_HAS(api, operator_info_free) ||
      !API_HAS(api, operator_new) || !API_HAS(api, operator_free)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  if (require_whole_object &&
      ((api->feature_bits & OPENDAL_MBT_FEATURE_WHOLE_OBJECT) == 0 ||
       !API_HAS(api, operator_check) || !API_HAS(api, operator_exists) ||
       !API_HAS(api, operator_stat) || !API_HAS(api, operator_read) ||
       !API_HAS(api, operator_write) || !API_HAS(api, operator_create_dir) ||
       !API_HAS(api, operator_delete) || !API_HAS(api, operator_copy) ||
       !API_HAS(api, operator_rename))) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_listing_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if ((api->feature_bits & OPENDAL_MBT_FEATURE_LISTING) == 0 ||
      !API_HAS(api, operator_lister) || !API_HAS(api, lister_next) ||
      !API_HAS(api, lister_close) || !API_HAS(api, lister_free)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_reader_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if ((api->feature_bits & OPENDAL_MBT_FEATURE_RANDOM_READER) == 0 ||
      !API_HAS(api, operator_reader) || !API_HAS(api, reader_read) ||
      !API_HAS(api, reader_close) || !API_HAS(api, reader_free)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_read_stream_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if (api->max_output_bytes == 0 ||
      (api->feature_bits & OPENDAL_MBT_FEATURE_READ_STREAM) == 0 ||
      !API_HAS(api, operator_read_stream) || !API_HAS(api, read_stream_next) ||
      !API_HAS(api, read_stream_close) || !API_HAS(api, read_stream_free)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_writer_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if ((api->feature_bits & OPENDAL_MBT_FEATURE_CHUNKED_WRITER) == 0 ||
      !API_HAS(api, operator_writer) || !API_HAS(api, writer_write) ||
      !API_HAS(api, writer_close) || !API_HAS(api, writer_free)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_writer_abort_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_writer_api(api);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if ((api->feature_bits & OPENDAL_MBT_FEATURE_WRITER_ABORT) == 0 ||
      !API_HAS(api, writer_abort)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_s3_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if (api->max_output_bytes == 0 ||
      (api->feature_bits & OPENDAL_MBT_FEATURE_S3) == 0 ||
      !API_HAS(api, operator_s3)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_presign_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if (api->max_output_bytes == 0 ||
      (api->feature_bits & OPENDAL_MBT_FEATURE_PRESIGN) == 0 ||
      !API_HAS(api, operator_presign_read) ||
      !API_HAS(api, operator_presign_write) ||
      !API_HAS(api, operator_presign_stat) ||
      !API_HAS(api, presigned_request_view) ||
      !API_HAS(api, presigned_request_header_view) ||
      !API_HAS(api, presigned_request_free)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_layers_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if ((api->feature_bits & OPENDAL_MBT_FEATURE_LAYERS) == 0 ||
      !API_HAS(api, operator_with_timeout) ||
      !API_HAS(api, operator_with_retry)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t
load_concurrency_limit_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if ((api->feature_bits & OPENDAL_MBT_FEATURE_CONCURRENCY_LIMIT) == 0 ||
      !API_HAS(api, operator_with_concurrency_limit)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_batch_delete_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if (api->max_output_bytes == 0 ||
      (api->feature_bits & OPENDAL_MBT_FEATURE_BATCH_DELETE) == 0 ||
      !API_HAS(api, operator_delete_many)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_copier_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if (api->max_output_bytes == 0 ||
      (api->feature_bits & OPENDAL_MBT_FEATURE_COPIER) == 0 ||
      !API_HAS(api, operator_copier) || !API_HAS(api, copier_next) ||
      !API_HAS(api, copier_finish) || !API_HAS(api, copier_abort) ||
      !API_HAS(api, copier_free)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static opendal_mbt_status_t load_async_api(opendal_mbt_api_v1_t *api) {
  opendal_mbt_status_t status = load_api(api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    return status;
  }
  if (api->max_output_bytes == 0 ||
      (api->feature_bits & OPENDAL_MBT_FEATURE_ASYNC) == 0 ||
      !API_HAS(api, async_operator_read_start) ||
      !API_HAS(api, async_operator_read_stream_start) ||
      !API_HAS(api, async_read_stream_next_start) ||
      !API_HAS(api, async_read_stream_close) ||
      !API_HAS(api, async_read_stream_free) ||
      !API_HAS(api, async_operator_writer_start) ||
      !API_HAS(api, async_writer_write_start) ||
      !API_HAS(api, async_writer_finish_start) ||
      !API_HAS(api, async_writer_abort_start) ||
      !API_HAS(api, async_writer_free) ||
      !API_HAS(api, async_task_cancel) ||
      !API_HAS(api, async_task_take_buffer) ||
      !API_HAS(api, async_task_take_metadata) ||
      !API_HAS(api, async_task_take_read_stream) ||
      !API_HAS(api, async_task_take_writer) ||
      !API_HAS(api, async_task_take_unit) ||
      !API_HAS(api, async_task_free)) {
    return OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return OPENDAL_MBT_STATUS_OK;
}

static void result_set_local_error(moonbit_opendal_result_t *result,
                                   opendal_mbt_error_kind_t kind,
                                   const char *kind_name,
                                   const char *message) {
  result->status = OPENDAL_MBT_STATUS_ERROR;
  result->local_kind = kind;
  result->local_error_status = OPENDAL_MBT_ERROR_STATUS_PERMANENT;
  result->local_kind_name = kind_name;
  result->local_message = message;
}

static moonbit_bytes_t copy_bytes(const uint8_t *data, uint64_t len) {
  moonbit_bytes_t output;
  if (len > (uint64_t)INT32_MAX || (len != 0 && data == NULL)) {
    return moonbit_make_bytes(0, 0);
  }
  output = moonbit_make_bytes((int32_t)len, 0);
  if (len != 0) {
    memcpy(output, data, (size_t)len);
  }
  return output;
}

static moonbit_bytes_t copy_c_string(const char *text) {
  if (text == NULL) {
    return moonbit_make_bytes(0, 0);
  }
  return copy_bytes((const uint8_t *)text, (uint64_t)strlen(text));
}

static bool valid_utf8(const uint8_t *data, uint64_t len) {
  uint64_t i = 0;
  if (len != 0 && data == NULL) {
    return false;
  }
  while (i < len) {
    uint8_t first = data[i];
    uint32_t codepoint;
    uint64_t width;
    if (first <= UINT8_C(0x7f)) {
      i += 1;
      continue;
    }
    if (first >= UINT8_C(0xc2) && first <= UINT8_C(0xdf)) {
      codepoint = (uint32_t)(first & UINT8_C(0x1f));
      width = 2;
    } else if (first >= UINT8_C(0xe0) && first <= UINT8_C(0xef)) {
      codepoint = (uint32_t)(first & UINT8_C(0x0f));
      width = 3;
    } else if (first >= UINT8_C(0xf0) && first <= UINT8_C(0xf4)) {
      codepoint = (uint32_t)(first & UINT8_C(0x07));
      width = 4;
    } else {
      return false;
    }
    if (width > len - i) {
      return false;
    }
    for (uint64_t j = 1; j < width; ++j) {
      uint8_t next = data[i + j];
      if ((next & UINT8_C(0xc0)) != UINT8_C(0x80)) {
        return false;
      }
      codepoint = (codepoint << 6) | (uint32_t)(next & UINT8_C(0x3f));
    }
    if ((width == 2 && codepoint < UINT32_C(0x80)) ||
        (width == 3 && codepoint < UINT32_C(0x800)) ||
        (width == 4 && codepoint < UINT32_C(0x10000)) ||
        (codepoint >= UINT32_C(0xd800) && codepoint <= UINT32_C(0xdfff)) ||
        codepoint > UINT32_C(0x10ffff)) {
      return false;
    }
    i += width;
  }
  return true;
}

static utf16_result_t utf16_to_utf8(moonbit_string_t input,
                                     owned_utf8_t *output) {
  int32_t input_len;
  uint64_t capacity;
  uint64_t written = 0;
  output->data = NULL;
  output->len = 0;
  if (input == NULL) {
    return UTF16_INVALID;
  }
  input_len = Moonbit_array_length(input);
  if (input_len < 0) {
    return UTF16_INVALID;
  }
  capacity = (uint64_t)(uint32_t)input_len * UINT64_C(3);
  if (capacity > (uint64_t)SIZE_MAX) {
    return UTF16_NO_MEMORY;
  }
  if (capacity != 0) {
    output->data = (uint8_t *)malloc((size_t)capacity);
    if (output->data == NULL) {
      return UTF16_NO_MEMORY;
    }
  }
  for (int32_t i = 0; i < input_len; ++i) {
    uint32_t codepoint = input[i];
    if (codepoint >= UINT32_C(0xd800) && codepoint <= UINT32_C(0xdbff)) {
      uint32_t low;
      if (i + 1 >= input_len) {
        free(output->data);
        output->data = NULL;
        return UTF16_INVALID;
      }
      low = input[i + 1];
      if (low < UINT32_C(0xdc00) || low > UINT32_C(0xdfff)) {
        free(output->data);
        output->data = NULL;
        return UTF16_INVALID;
      }
      codepoint = UINT32_C(0x10000) +
                  ((codepoint - UINT32_C(0xd800)) << 10) +
                  (low - UINT32_C(0xdc00));
      i += 1;
    } else if (codepoint >= UINT32_C(0xdc00) &&
               codepoint <= UINT32_C(0xdfff)) {
      free(output->data);
      output->data = NULL;
      return UTF16_INVALID;
    }
    if (codepoint <= UINT32_C(0x7f)) {
      output->data[written++] = (uint8_t)codepoint;
    } else if (codepoint <= UINT32_C(0x7ff)) {
      output->data[written++] =
          (uint8_t)(UINT32_C(0xc0) | (codepoint >> 6));
      output->data[written++] =
          (uint8_t)(UINT32_C(0x80) | (codepoint & UINT32_C(0x3f)));
    } else if (codepoint <= UINT32_C(0xffff)) {
      output->data[written++] =
          (uint8_t)(UINT32_C(0xe0) | (codepoint >> 12));
      output->data[written++] = (uint8_t)(
          UINT32_C(0x80) | ((codepoint >> 6) & UINT32_C(0x3f)));
      output->data[written++] =
          (uint8_t)(UINT32_C(0x80) | (codepoint & UINT32_C(0x3f)));
    } else {
      output->data[written++] =
          (uint8_t)(UINT32_C(0xf0) | (codepoint >> 18));
      output->data[written++] = (uint8_t)(
          UINT32_C(0x80) | ((codepoint >> 12) & UINT32_C(0x3f)));
      output->data[written++] = (uint8_t)(
          UINT32_C(0x80) | ((codepoint >> 6) & UINT32_C(0x3f)));
      output->data[written++] =
          (uint8_t)(UINT32_C(0x80) | (codepoint & UINT32_C(0x3f)));
    }
  }
  output->len = written;
  return UTF16_OK;
}

static void owned_utf8_free(owned_utf8_t *value) {
  free(value->data);
  value->data = NULL;
  value->len = 0;
}

static opendal_mbt_bytes_view_v1_t owned_utf8_view(
    const owned_utf8_t *value) {
  opendal_mbt_bytes_view_v1_t view;
  view.data = value->data;
  view.len = value->len;
  return view;
}

static bool convert_optional_utf8(moonbit_opendal_result_t *result,
                                  bool present, moonbit_string_t input,
                                  const char *invalid_message,
                                  const char *allocation_message,
                                  owned_utf8_t *output) {
  utf16_result_t conversion;
  if (!present) {
    return true;
  }
  conversion = utf16_to_utf8(input, output);
  if (conversion == UTF16_OK) {
    return true;
  }
  result_set_local_error(
      result,
      conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                  : OPENDAL_MBT_ERROR_UNEXPECTED,
      conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
      conversion == UTF16_INVALID ? invalid_message : allocation_message);
  return false;
}

static bool prepare_write_options(
    moonbit_opendal_result_t *result, int32_t append,
    int32_t has_content_type, moonbit_string_t content_type,
    int32_t has_content_disposition, moonbit_string_t content_disposition,
    int32_t has_content_encoding, moonbit_string_t content_encoding,
    int32_t has_cache_control, moonbit_string_t cache_control,
    int32_t has_if_match, moonbit_string_t if_match,
    int32_t has_if_none_match, moonbit_string_t if_none_match,
    owned_utf8_t option_utf8[6], opendal_mbt_write_options_v1_t *options) {
  if ((append != 0 && append != 1) ||
      (has_content_type != 0 && has_content_type != 1) ||
      (has_content_disposition != 0 && has_content_disposition != 1) ||
      (has_content_encoding != 0 && has_content_encoding != 1) ||
      (has_cache_control != 0 && has_cache_control != 1) ||
      (has_if_match != 0 && has_if_match != 1) ||
      (has_if_none_match != 0 && has_if_none_match != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return false;
  }
  if (!convert_optional_utf8(
          result, has_content_type != 0, content_type,
          "write content_type contains invalid UTF-16",
          "unable to allocate UTF-8 write content_type", &option_utf8[0]) ||
      !convert_optional_utf8(
          result, has_content_disposition != 0, content_disposition,
          "write content_disposition contains invalid UTF-16",
          "unable to allocate UTF-8 write content_disposition",
          &option_utf8[1]) ||
      !convert_optional_utf8(
          result, has_content_encoding != 0, content_encoding,
          "write content_encoding contains invalid UTF-16",
          "unable to allocate UTF-8 write content_encoding", &option_utf8[2]) ||
      !convert_optional_utf8(
          result, has_cache_control != 0, cache_control,
          "write cache_control contains invalid UTF-16",
          "unable to allocate UTF-8 write cache_control", &option_utf8[3]) ||
      !convert_optional_utf8(
          result, has_if_match != 0, if_match,
          "write if_match contains invalid UTF-16",
          "unable to allocate UTF-8 write if_match", &option_utf8[4]) ||
      !convert_optional_utf8(
          result, has_if_none_match != 0, if_none_match,
          "write if_none_match contains invalid UTF-16",
          "unable to allocate UTF-8 write if_none_match", &option_utf8[5])) {
    return false;
  }
  memset(options, 0, sizeof(*options));
  options->struct_size = (uint32_t)sizeof(*options);
  options->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  if (append != 0) {
    options->flags |= OPENDAL_MBT_WRITE_APPEND;
  }
  if (has_content_type != 0) {
    options->present_bits |= OPENDAL_MBT_WRITE_CONTENT_TYPE_PRESENT;
    options->content_type = owned_utf8_view(&option_utf8[0]);
  }
  if (has_content_disposition != 0) {
    options->present_bits |= OPENDAL_MBT_WRITE_CONTENT_DISPOSITION_PRESENT;
    options->content_disposition = owned_utf8_view(&option_utf8[1]);
  }
  if (has_content_encoding != 0) {
    options->present_bits |= OPENDAL_MBT_WRITE_CONTENT_ENCODING_PRESENT;
    options->content_encoding = owned_utf8_view(&option_utf8[2]);
  }
  if (has_cache_control != 0) {
    options->present_bits |= OPENDAL_MBT_WRITE_CACHE_CONTROL_PRESENT;
    options->cache_control = owned_utf8_view(&option_utf8[3]);
  }
  if (has_if_match != 0) {
    options->present_bits |= OPENDAL_MBT_WRITE_IF_MATCH_PRESENT;
    options->if_match = owned_utf8_view(&option_utf8[4]);
  }
  if (has_if_none_match != 0) {
    options->present_bits |= OPENDAL_MBT_WRITE_IF_NONE_MATCH_PRESENT;
    options->if_none_match = owned_utf8_view(&option_utf8[5]);
  }
  return true;
}

static bool prepare_stat_options(
    moonbit_opendal_result_t *result, int32_t has_version,
    moonbit_string_t version, int32_t has_if_match, moonbit_string_t if_match,
    int32_t has_if_none_match, moonbit_string_t if_none_match,
    owned_utf8_t option_utf8[3], opendal_mbt_stat_options_v1_t *options) {
  if ((has_version != 0 && has_version != 1) ||
      (has_if_match != 0 && has_if_match != 1) ||
      (has_if_none_match != 0 && has_if_none_match != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return false;
  }
  if (!convert_optional_utf8(
          result, has_version != 0, version,
          "stat version contains invalid UTF-16",
          "unable to allocate UTF-8 stat version", &option_utf8[0]) ||
      !convert_optional_utf8(
          result, has_if_match != 0, if_match,
          "stat if_match contains invalid UTF-16",
          "unable to allocate UTF-8 stat if_match", &option_utf8[1]) ||
      !convert_optional_utf8(
          result, has_if_none_match != 0, if_none_match,
          "stat if_none_match contains invalid UTF-16",
          "unable to allocate UTF-8 stat if_none_match", &option_utf8[2])) {
    return false;
  }
  memset(options, 0, sizeof(*options));
  options->struct_size = (uint32_t)sizeof(*options);
  options->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  if (has_version != 0) {
    options->present_bits |= OPENDAL_MBT_STAT_VERSION_PRESENT;
    options->version = owned_utf8_view(&option_utf8[0]);
  }
  if (has_if_match != 0) {
    options->present_bits |= OPENDAL_MBT_STAT_IF_MATCH_PRESENT;
    options->if_match = owned_utf8_view(&option_utf8[1]);
  }
  if (has_if_none_match != 0) {
    options->present_bits |= OPENDAL_MBT_STAT_IF_NONE_MATCH_PRESENT;
    options->if_none_match = owned_utf8_view(&option_utf8[2]);
  }
  return true;
}

static bool prepare_read_options(
    moonbit_opendal_result_t *result, uint32_t range_kind,
    uint64_t range_offset, uint64_t range_length, int32_t has_version,
    moonbit_string_t version, int32_t has_if_match, moonbit_string_t if_match,
    int32_t has_if_none_match, moonbit_string_t if_none_match,
    owned_utf8_t option_utf8[3], opendal_mbt_read_options_v1_t *options) {
  if ((has_version != 0 && has_version != 1) ||
      (has_if_match != 0 && has_if_match != 1) ||
      (has_if_none_match != 0 && has_if_none_match != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return false;
  }
  if (!convert_optional_utf8(
          result, has_version != 0, version,
          "read version contains invalid UTF-16",
          "unable to allocate UTF-8 read version", &option_utf8[0]) ||
      !convert_optional_utf8(
          result, has_if_match != 0, if_match,
          "read if_match contains invalid UTF-16",
          "unable to allocate UTF-8 read if_match", &option_utf8[1]) ||
      !convert_optional_utf8(
          result, has_if_none_match != 0, if_none_match,
          "read if_none_match contains invalid UTF-16",
          "unable to allocate UTF-8 read if_none_match", &option_utf8[2])) {
    return false;
  }
  memset(options, 0, sizeof(*options));
  options->struct_size = (uint32_t)sizeof(*options);
  options->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options->range.struct_size = (uint32_t)sizeof(options->range);
  options->range.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options->range.kind = range_kind;
  options->range.offset = range_offset;
  options->range.length = range_length;
  if (has_version != 0) {
    options->present_bits |= OPENDAL_MBT_READ_VERSION_PRESENT;
    options->version = owned_utf8_view(&option_utf8[0]);
  }
  if (has_if_match != 0) {
    options->present_bits |= OPENDAL_MBT_READ_IF_MATCH_PRESENT;
    options->if_match = owned_utf8_view(&option_utf8[1]);
  }
  if (has_if_none_match != 0) {
    options->present_bits |= OPENDAL_MBT_READ_IF_NONE_MATCH_PRESENT;
    options->if_none_match = owned_utf8_view(&option_utf8[2]);
  }
  return true;
}

static bool prepare_async_read_options(
    moonbit_opendal_result_t *result, uint32_t range_kind,
    uint64_t range_offset, uint64_t range_length, int32_t has_version,
    moonbit_string_t version, int32_t has_if_match, moonbit_string_t if_match,
    int32_t has_if_none_match, moonbit_string_t if_none_match,
    owned_utf8_t option_utf8[3], opendal_mbt_read_options_v1_t *options) {
  if ((has_version != 0 && has_version != 1) ||
      (has_if_match != 0 && has_if_match != 1) ||
      (has_if_none_match != 0 && has_if_none_match != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return false;
  }
  if (!convert_optional_utf8(
          result, has_version != 0, version,
          "async read version contains invalid UTF-16",
          "unable to allocate UTF-8 async read version", &option_utf8[0]) ||
      !convert_optional_utf8(
          result, has_if_match != 0, if_match,
          "async read if_match contains invalid UTF-16",
          "unable to allocate UTF-8 async read if_match", &option_utf8[1]) ||
      !convert_optional_utf8(
          result, has_if_none_match != 0, if_none_match,
          "async read if_none_match contains invalid UTF-16",
          "unable to allocate UTF-8 async read if_none_match",
          &option_utf8[2])) {
    return false;
  }
  memset(options, 0, sizeof(*options));
  options->struct_size = (uint32_t)sizeof(*options);
  options->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options->range.struct_size = (uint32_t)sizeof(options->range);
  options->range.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options->range.kind = range_kind;
  options->range.offset = range_offset;
  options->range.length = range_length;
  if (has_version != 0) {
    options->present_bits |= OPENDAL_MBT_READ_VERSION_PRESENT;
    options->version = owned_utf8_view(&option_utf8[0]);
  }
  if (has_if_match != 0) {
    options->present_bits |= OPENDAL_MBT_READ_IF_MATCH_PRESENT;
    options->if_match = owned_utf8_view(&option_utf8[1]);
  }
  if (has_if_none_match != 0) {
    options->present_bits |= OPENDAL_MBT_READ_IF_NONE_MATCH_PRESENT;
    options->if_none_match = owned_utf8_view(&option_utf8[2]);
  }
  return true;
}

static bool prepare_async_read_stream_options(
    moonbit_opendal_result_t *result, uint32_t range_kind,
    uint64_t range_offset, uint64_t range_length, int32_t chunk_size,
    int32_t has_version, moonbit_string_t version, int32_t has_if_match,
    moonbit_string_t if_match, int32_t has_if_none_match,
    moonbit_string_t if_none_match, owned_utf8_t option_utf8[3],
    opendal_mbt_read_stream_options_v1_t *options) {
  if ((has_version != 0 && has_version != 1) ||
      (has_if_match != 0 && has_if_match != 1) ||
      (has_if_none_match != 0 && has_if_none_match != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return false;
  }
  if (chunk_size <= 0) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                           "InvalidArgument",
                           "async read stream chunk_size must be positive");
    return false;
  }
  if (!convert_optional_utf8(
          result, has_version != 0, version,
          "async read stream version contains invalid UTF-16",
          "unable to allocate UTF-8 async read stream version",
          &option_utf8[0]) ||
      !convert_optional_utf8(
          result, has_if_match != 0, if_match,
          "async read stream if_match contains invalid UTF-16",
          "unable to allocate UTF-8 async read stream if_match",
          &option_utf8[1]) ||
      !convert_optional_utf8(
          result, has_if_none_match != 0, if_none_match,
          "async read stream if_none_match contains invalid UTF-16",
          "unable to allocate UTF-8 async read stream if_none_match",
          &option_utf8[2])) {
    return false;
  }
  memset(options, 0, sizeof(*options));
  options->struct_size = (uint32_t)sizeof(*options);
  options->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options->range.struct_size = (uint32_t)sizeof(options->range);
  options->range.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options->range.kind = range_kind;
  options->range.offset = range_offset;
  options->range.length = range_length;
  options->chunk_size = (uint64_t)(uint32_t)chunk_size;
  if (has_version != 0) {
    options->present_bits |= OPENDAL_MBT_READ_STREAM_VERSION_PRESENT;
    options->version = owned_utf8_view(&option_utf8[0]);
  }
  if (has_if_match != 0) {
    options->present_bits |= OPENDAL_MBT_READ_STREAM_IF_MATCH_PRESENT;
    options->if_match = owned_utf8_view(&option_utf8[1]);
  }
  if (has_if_none_match != 0) {
    options->present_bits |= OPENDAL_MBT_READ_STREAM_IF_NONE_MATCH_PRESENT;
    options->if_none_match = owned_utf8_view(&option_utf8[2]);
  }
  return true;
}

static void release_result_payload(moonbit_opendal_result_t *result) {
  opendal_mbt_api_v1_t api;
  if (load_api(&api, false) != OPENDAL_MBT_STATUS_OK) {
    return;
  }
  if (result->error != NULL) {
    api.error_free(result->error);
    result->error = NULL;
  }
  if (result->buffer != NULL) {
    api.buffer_free(result->buffer);
    result->buffer = NULL;
  }
  if (result->metadata != NULL) {
    api.metadata_free(result->metadata);
    result->metadata = NULL;
  }
  if (result->entry != NULL) {
    api.entry_free(result->entry);
    result->entry = NULL;
  }
  if (result->lister != NULL && API_HAS(&api, lister_free)) {
    api.lister_free(result->lister);
    result->lister = NULL;
  }
  if (result->reader != NULL && API_HAS(&api, reader_free)) {
    api.reader_free(result->reader);
    result->reader = NULL;
  }
  if (result->read_stream != NULL &&
      (api.feature_bits & OPENDAL_MBT_FEATURE_READ_STREAM) != 0 &&
      API_HAS(&api, read_stream_free)) {
    api.read_stream_free(result->read_stream);
    result->read_stream = NULL;
  }
  if (result->writer != NULL && API_HAS(&api, writer_free)) {
    api.writer_free(result->writer);
    result->writer = NULL;
  }
  if (result->copier != NULL &&
      (api.feature_bits & OPENDAL_MBT_FEATURE_COPIER) != 0 &&
      API_HAS(&api, copier_free)) {
    api.copier_free(result->copier);
    result->copier = NULL;
  }
  if (result->presigned_request != NULL &&
      (api.feature_bits & OPENDAL_MBT_FEATURE_PRESIGN) != 0 &&
      API_HAS(&api, presigned_request_free)) {
    api.presigned_request_free(result->presigned_request);
    result->presigned_request = NULL;
  }
  if (result->async_task != NULL &&
      (api.feature_bits & OPENDAL_MBT_FEATURE_ASYNC) != 0 &&
      API_HAS(&api, async_task_free)) {
    api.async_task_free(result->async_task);
    result->async_task = NULL;
  }
  if (result->async_read_stream != NULL &&
      (api.feature_bits & OPENDAL_MBT_FEATURE_ASYNC) != 0 &&
      API_HAS(&api, async_read_stream_free)) {
    api.async_read_stream_free(result->async_read_stream);
    result->async_read_stream = NULL;
  }
  if (result->async_writer != NULL &&
      (api.feature_bits & OPENDAL_MBT_FEATURE_ASYNC) != 0 &&
      API_HAS(&api, async_writer_free)) {
    api.async_writer_free(result->async_writer);
    result->async_writer = NULL;
  }
  if (result->info != NULL) {
    api.operator_info_free(result->info);
    result->info = NULL;
  }
  if (result->operator_ != NULL) {
    api.operator_free(result->operator_);
    result->operator_ = NULL;
  }
}

static void result_finalize(void *payload) {
  release_result_payload((moonbit_opendal_result_t *)payload);
}

static moonbit_opendal_result_t *result_new(void) {
  moonbit_opendal_result_t *result =
      (moonbit_opendal_result_t *)moonbit_make_external_object(
          result_finalize, (uint32_t)sizeof(moonbit_opendal_result_t));
  memset(result, 0, sizeof(*result));
  result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  return result;
}

static void operator_finalize(void *payload) {
  moonbit_opendal_operator_t *operator_ =
      (moonbit_opendal_operator_t *)payload;
  opendal_mbt_api_v1_t api;
  if (load_api(&api, false) != OPENDAL_MBT_STATUS_OK) {
    return;
  }
  if (operator_->info != NULL) {
    api.operator_info_free(operator_->info);
    operator_->info = NULL;
  }
  if (operator_->operator_ != NULL) {
    api.operator_free(operator_->operator_);
    operator_->operator_ = NULL;
  }
}

static moonbit_opendal_operator_t *operator_new_external(void) {
  moonbit_opendal_operator_t *operator_ =
      (moonbit_opendal_operator_t *)moonbit_make_external_object(
          operator_finalize, (uint32_t)sizeof(moonbit_opendal_operator_t));
  memset(operator_, 0, sizeof(*operator_));
  return operator_;
}

static void metadata_finalize(void *payload) {
  moonbit_opendal_metadata_t *metadata = (moonbit_opendal_metadata_t *)payload;
  opendal_mbt_api_v1_t api;
  if (load_api(&api, false) == OPENDAL_MBT_STATUS_OK) {
    if (metadata->metadata != NULL) {
      api.metadata_free(metadata->metadata);
      metadata->metadata = NULL;
    }
    if (metadata->entry != NULL) {
      api.entry_free(metadata->entry);
      metadata->entry = NULL;
    }
  }
}

static moonbit_opendal_metadata_t *metadata_new_external(void) {
  moonbit_opendal_metadata_t *metadata =
      (moonbit_opendal_metadata_t *)moonbit_make_external_object(
          metadata_finalize, (uint32_t)sizeof(moonbit_opendal_metadata_t));
  memset(metadata, 0, sizeof(*metadata));
  return metadata;
}

static void lister_finalize(void *payload) {
  moonbit_opendal_lister_t *lister = (moonbit_opendal_lister_t *)payload;
  opendal_mbt_api_v1_t api;
  if (lister->lister != NULL &&
      load_listing_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.lister_free(lister->lister);
    lister->lister = NULL;
  }
}

static moonbit_opendal_lister_t *lister_new_external(void) {
  moonbit_opendal_lister_t *lister =
      (moonbit_opendal_lister_t *)moonbit_make_external_object(
          lister_finalize, (uint32_t)sizeof(moonbit_opendal_lister_t));
  lister->lister = NULL;
  return lister;
}

static void reader_finalize(void *payload) {
  moonbit_opendal_reader_t *reader = (moonbit_opendal_reader_t *)payload;
  opendal_mbt_api_v1_t api;
  if (reader->reader != NULL &&
      load_reader_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.reader_free(reader->reader);
    reader->reader = NULL;
  }
}

static moonbit_opendal_reader_t *reader_new_external(void) {
  moonbit_opendal_reader_t *reader =
      (moonbit_opendal_reader_t *)moonbit_make_external_object(
          reader_finalize, (uint32_t)sizeof(moonbit_opendal_reader_t));
  reader->reader = NULL;
  return reader;
}

static void read_stream_finalize(void *payload) {
  moonbit_opendal_read_stream_t *stream =
      (moonbit_opendal_read_stream_t *)payload;
  opendal_mbt_api_v1_t api;
  if (stream->stream != NULL &&
      load_read_stream_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.read_stream_free(stream->stream);
    stream->stream = NULL;
  }
}

static moonbit_opendal_read_stream_t *read_stream_new_external(void) {
  moonbit_opendal_read_stream_t *stream =
      (moonbit_opendal_read_stream_t *)moonbit_make_external_object(
          read_stream_finalize,
          (uint32_t)sizeof(moonbit_opendal_read_stream_t));
  stream->stream = NULL;
  return stream;
}

static void writer_finalize(void *payload) {
  moonbit_opendal_writer_t *writer = (moonbit_opendal_writer_t *)payload;
  opendal_mbt_api_v1_t api;
  if (writer->writer != NULL &&
      load_writer_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.writer_free(writer->writer);
    writer->writer = NULL;
  }
}

static moonbit_opendal_writer_t *writer_new_external(void) {
  moonbit_opendal_writer_t *writer =
      (moonbit_opendal_writer_t *)moonbit_make_external_object(
          writer_finalize, (uint32_t)sizeof(moonbit_opendal_writer_t));
  writer->writer = NULL;
  return writer;
}

static void copier_finalize(void *payload) {
  moonbit_opendal_copier_t *copier = (moonbit_opendal_copier_t *)payload;
  opendal_mbt_api_v1_t api;
  if (copier->copier != NULL &&
      load_copier_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.copier_free(copier->copier);
    copier->copier = NULL;
  }
}

static moonbit_opendal_copier_t *copier_new_external(void) {
  moonbit_opendal_copier_t *copier =
      (moonbit_opendal_copier_t *)moonbit_make_external_object(
          copier_finalize, (uint32_t)sizeof(moonbit_opendal_copier_t));
  copier->copier = NULL;
  return copier;
}

static void async_task_finalize(void *payload) {
  moonbit_opendal_async_task_t *task =
      (moonbit_opendal_async_task_t *)payload;
  opendal_mbt_api_v1_t api;
  if (task->task != NULL && load_async_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.async_task_cancel(task->task);
    api.async_task_free(task->task);
    task->task = NULL;
  }
}

static moonbit_opendal_async_task_t *async_task_new_external(void) {
  moonbit_opendal_async_task_t *task =
      (moonbit_opendal_async_task_t *)moonbit_make_external_object(
          async_task_finalize,
          (uint32_t)sizeof(moonbit_opendal_async_task_t));
  memset(task, 0, sizeof(*task));
  return task;
}

static void async_read_stream_finalize(void *payload) {
  moonbit_opendal_async_read_stream_t *stream =
      (moonbit_opendal_async_read_stream_t *)payload;
  opendal_mbt_api_v1_t api;
  if (stream->stream != NULL &&
      load_async_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.async_read_stream_free(stream->stream);
    stream->stream = NULL;
  }
}

static moonbit_opendal_async_read_stream_t *
async_read_stream_new_external(void) {
  moonbit_opendal_async_read_stream_t *stream =
      (moonbit_opendal_async_read_stream_t *)moonbit_make_external_object(
          async_read_stream_finalize,
          (uint32_t)sizeof(moonbit_opendal_async_read_stream_t));
  stream->stream = NULL;
  return stream;
}

static void async_writer_finalize(void *payload) {
  moonbit_opendal_async_writer_t *writer =
      (moonbit_opendal_async_writer_t *)payload;
  opendal_mbt_api_v1_t api;
  if (writer->writer != NULL &&
      load_async_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.async_writer_free(writer->writer);
    writer->writer = NULL;
  }
}

static moonbit_opendal_async_writer_t *async_writer_new_external(void) {
  moonbit_opendal_async_writer_t *writer =
      (moonbit_opendal_async_writer_t *)moonbit_make_external_object(
          async_writer_finalize,
          (uint32_t)sizeof(moonbit_opendal_async_writer_t));
  writer->writer = NULL;
  return writer;
}

static void entry_finalize(void *payload) {
  moonbit_opendal_entry_t *entry = (moonbit_opendal_entry_t *)payload;
  opendal_mbt_api_v1_t api;
  if (entry->entry != NULL &&
      load_api(&api, false) == OPENDAL_MBT_STATUS_OK) {
    api.entry_free(entry->entry);
    entry->entry = NULL;
  }
}

static moonbit_opendal_entry_t *entry_new_external(void) {
  moonbit_opendal_entry_t *entry =
      (moonbit_opendal_entry_t *)moonbit_make_external_object(
          entry_finalize, (uint32_t)sizeof(moonbit_opendal_entry_t));
  entry->entry = NULL;
  return entry;
}

static void presigned_request_finalize(void *payload) {
  moonbit_opendal_presigned_request_t *request =
      (moonbit_opendal_presigned_request_t *)payload;
  opendal_mbt_api_v1_t api;
  if (request->request != NULL &&
      load_presign_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.presigned_request_free(request->request);
    request->request = NULL;
  }
}

static moonbit_opendal_presigned_request_t *
presigned_request_new_external(void) {
  moonbit_opendal_presigned_request_t *request =
      (moonbit_opendal_presigned_request_t *)moonbit_make_external_object(
          presigned_request_finalize,
          (uint32_t)sizeof(moonbit_opendal_presigned_request_t));
  request->request = NULL;
  return request;
}

static void capability_finalize(void *payload) { (void)payload; }

static moonbit_opendal_capability_t *capability_new_external(void) {
  moonbit_opendal_capability_t *capability =
      (moonbit_opendal_capability_t *)moonbit_make_external_object(
          capability_finalize,
          (uint32_t)sizeof(moonbit_opendal_capability_t));
  memset(capability, 0, sizeof(*capability));
  return capability;
}

static bool fill_operator_info_view(
    const moonbit_opendal_operator_t *operator_,
    opendal_mbt_operator_info_view_v1_t *view) {
  opendal_mbt_api_v1_t api;
  if (operator_ == NULL || operator_->info == NULL ||
      load_api(&api, false) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  memset(view, 0, sizeof(*view));
  view->struct_size = (uint32_t)sizeof(*view);
  view->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  return api.operator_info_view(operator_->info, view) ==
         OPENDAL_MBT_STATUS_OK;
}

static bool validate_operator_info(opendal_mbt_operator_info_v1_t *info) {
  opendal_mbt_api_v1_t api;
  opendal_mbt_operator_info_view_v1_t view;
  if (info == NULL || load_api(&api, false) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  memset(&view, 0, sizeof(view));
  view.struct_size = (uint32_t)sizeof(view);
  view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  if (api.operator_info_view(info, &view) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  return view.scheme.len <= (uint64_t)INT32_MAX &&
         view.root.len <= (uint64_t)INT32_MAX &&
         view.name.len <= (uint64_t)INT32_MAX &&
         valid_utf8(view.scheme.data, view.scheme.len) &&
         valid_utf8(view.root.data, view.root.len) &&
         valid_utf8(view.name.data, view.name.len);
}

static bool valid_optional_text(opendal_mbt_bytes_view_v1_t value,
                                bool present) {
  if (!present) {
    return value.data == NULL && value.len == 0;
  }
  return value.len <= (uint64_t)INT32_MAX &&
         valid_utf8(value.data, value.len);
}

static bool validate_metadata_view(
    const opendal_mbt_metadata_view_v1_t *view) {
  const uint64_t known_present_bits =
      OPENDAL_MBT_METADATA_IS_CURRENT_PRESENT |
      OPENDAL_MBT_METADATA_LAST_MODIFIED_PRESENT |
      OPENDAL_MBT_METADATA_CACHE_CONTROL_PRESENT |
      OPENDAL_MBT_METADATA_CONTENT_DISPOSITION_PRESENT |
      OPENDAL_MBT_METADATA_CONTENT_ENCODING_PRESENT |
      OPENDAL_MBT_METADATA_CONTENT_MD5_PRESENT |
      OPENDAL_MBT_METADATA_CONTENT_TYPE_PRESENT |
      OPENDAL_MBT_METADATA_ETAG_PRESENT |
      OPENDAL_MBT_METADATA_VERSION_PRESENT;
  if ((view->present_bits & ~known_present_bits) != 0 ||
      (view->is_current != OPENDAL_MBT_FALSE &&
       view->is_current != OPENDAL_MBT_TRUE) ||
      (view->is_deleted != OPENDAL_MBT_FALSE &&
       view->is_deleted != OPENDAL_MBT_TRUE) ||
      view->reserved0 != 0) {
    return false;
  }
  if ((view->present_bits & OPENDAL_MBT_METADATA_IS_CURRENT_PRESENT) == 0 &&
      view->is_current != OPENDAL_MBT_FALSE) {
    return false;
  }
  if ((view->present_bits & OPENDAL_MBT_METADATA_LAST_MODIFIED_PRESENT) != 0) {
    if (view->last_modified.nanoseconds >= UINT32_C(1000000000) ||
        view->last_modified.reserved0 != 0) {
      return false;
    }
  } else if (view->last_modified.unix_seconds != 0 ||
             view->last_modified.nanoseconds != 0 ||
             view->last_modified.reserved0 != 0) {
    return false;
  }
  return valid_optional_text(
             view->cache_control,
             (view->present_bits & OPENDAL_MBT_METADATA_CACHE_CONTROL_PRESENT) !=
                 0) &&
         valid_optional_text(
             view->content_disposition,
             (view->present_bits &
              OPENDAL_MBT_METADATA_CONTENT_DISPOSITION_PRESENT) != 0) &&
         valid_optional_text(
             view->content_encoding,
             (view->present_bits &
              OPENDAL_MBT_METADATA_CONTENT_ENCODING_PRESENT) != 0) &&
         valid_optional_text(
             view->content_md5,
             (view->present_bits & OPENDAL_MBT_METADATA_CONTENT_MD5_PRESENT) !=
                 0) &&
         valid_optional_text(
             view->content_type,
             (view->present_bits & OPENDAL_MBT_METADATA_CONTENT_TYPE_PRESENT) !=
                 0) &&
         valid_optional_text(
             view->etag,
             (view->present_bits & OPENDAL_MBT_METADATA_ETAG_PRESENT) != 0) &&
         valid_optional_text(
             view->version,
             (view->present_bits & OPENDAL_MBT_METADATA_VERSION_PRESENT) != 0);
}

static bool validate_metadata_snapshot(opendal_mbt_metadata_v1_t *metadata) {
  opendal_mbt_api_v1_t api;
  opendal_mbt_metadata_view_v1_t view;
  if (metadata == NULL || load_api(&api, false) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  memset(&view, 0, sizeof(view));
  view.struct_size = (uint32_t)sizeof(view);
  view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  return api.metadata_view(metadata, &view) == OPENDAL_MBT_STATUS_OK &&
         validate_metadata_view(&view);
}

static bool fill_entry_view(const moonbit_opendal_entry_t *entry,
                            opendal_mbt_entry_view_v1_t *view) {
  opendal_mbt_api_v1_t api;
  if (entry == NULL || entry->entry == NULL ||
      load_api(&api, false) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  memset(view, 0, sizeof(*view));
  view->struct_size = (uint32_t)sizeof(*view);
  view->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  return api.entry_view(entry->entry, view) == OPENDAL_MBT_STATUS_OK;
}

static bool validate_entry_snapshot(opendal_mbt_entry_v1_t *entry) {
  opendal_mbt_api_v1_t api;
  opendal_mbt_entry_view_v1_t entry_view;
  opendal_mbt_metadata_view_v1_t metadata_view;
  if (entry == NULL || load_api(&api, false) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  memset(&entry_view, 0, sizeof(entry_view));
  entry_view.struct_size = (uint32_t)sizeof(entry_view);
  entry_view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  memset(&metadata_view, 0, sizeof(metadata_view));
  metadata_view.struct_size = (uint32_t)sizeof(metadata_view);
  metadata_view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  return api.entry_view(entry, &entry_view) == OPENDAL_MBT_STATUS_OK &&
         entry_view.reserved0 == 0 &&
         entry_view.path.len <= (uint64_t)INT32_MAX &&
         entry_view.name.len <= (uint64_t)INT32_MAX &&
         valid_utf8(entry_view.path.data, entry_view.path.len) &&
         valid_utf8(entry_view.name.data, entry_view.name.len) &&
         api.entry_metadata_view(entry, &metadata_view) ==
             OPENDAL_MBT_STATUS_OK &&
         validate_metadata_view(&metadata_view);
}

static bool fill_presigned_request_view(
    const moonbit_opendal_presigned_request_t *request,
    opendal_mbt_presigned_request_view_v1_t *view) {
  opendal_mbt_api_v1_t api;
  if (request == NULL || request->request == NULL ||
      load_presign_api(&api) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  memset(view, 0, sizeof(*view));
  view->struct_size = (uint32_t)sizeof(*view);
  view->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  return api.presigned_request_view(request->request, view) ==
         OPENDAL_MBT_STATUS_OK;
}

static bool fill_presigned_header_view(
    const moonbit_opendal_presigned_request_t *request, uint64_t index,
    opendal_mbt_presigned_header_view_v1_t *view) {
  opendal_mbt_api_v1_t api;
  if (request == NULL || request->request == NULL ||
      load_presign_api(&api) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  memset(view, 0, sizeof(*view));
  view->struct_size = (uint32_t)sizeof(*view);
  view->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  return api.presigned_request_header_view(request->request, index, view) ==
         OPENDAL_MBT_STATUS_OK;
}

static bool validate_presigned_request_snapshot(
    opendal_mbt_presigned_request_v1_t *request) {
  opendal_mbt_api_v1_t api;
  opendal_mbt_presigned_request_view_v1_t request_view;
  if (request == NULL || load_presign_api(&api) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  memset(&request_view, 0, sizeof(request_view));
  request_view.struct_size = (uint32_t)sizeof(request_view);
  request_view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  if (api.presigned_request_view(request, &request_view) !=
          OPENDAL_MBT_STATUS_OK ||
      request_view.reserved0 != 0 ||
      request_view.method.len > (uint64_t)INT32_MAX ||
      request_view.uri.len > (uint64_t)INT32_MAX ||
      request_view.header_count > (uint64_t)INT32_MAX ||
      !valid_utf8(request_view.method.data, request_view.method.len) ||
      !valid_utf8(request_view.uri.data, request_view.uri.len)) {
    return false;
  }
  for (uint64_t index = 0; index < request_view.header_count; ++index) {
    opendal_mbt_presigned_header_view_v1_t header_view;
    memset(&header_view, 0, sizeof(header_view));
    header_view.struct_size = (uint32_t)sizeof(header_view);
    header_view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
    if (api.presigned_request_header_view(request, index, &header_view) !=
            OPENDAL_MBT_STATUS_OK ||
        header_view.reserved0 != 0 ||
        header_view.name.len > (uint64_t)INT32_MAX ||
        header_view.value.len > (uint64_t)INT32_MAX ||
        !valid_utf8(header_view.name.data, header_view.name.len) ||
        (header_view.value.len != 0 && header_view.value.data == NULL)) {
      return false;
    }
  }
  return true;
}

static bool fill_metadata_view(const moonbit_opendal_metadata_t *metadata,
                               opendal_mbt_metadata_view_v1_t *view) {
  opendal_mbt_api_v1_t api;
  if (metadata == NULL ||
      (metadata->metadata == NULL && metadata->entry == NULL) ||
      (metadata->metadata != NULL && metadata->entry != NULL) ||
      load_api(&api, false) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  memset(view, 0, sizeof(*view));
  view->struct_size = (uint32_t)sizeof(*view);
  view->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  if (metadata->metadata != NULL) {
    return api.metadata_view(metadata->metadata, view) ==
           OPENDAL_MBT_STATUS_OK;
  }
  return api.entry_metadata_view(metadata->entry, view) ==
         OPENDAL_MBT_STATUS_OK;
}

static bool fill_error_view(const moonbit_opendal_result_t *result,
                            opendal_mbt_error_view_v1_t *view) {
  opendal_mbt_api_v1_t api;
  if (result == NULL || result->error == NULL ||
      load_api(&api, false) != OPENDAL_MBT_STATUS_OK) {
    return false;
  }
  memset(view, 0, sizeof(*view));
  view->struct_size = (uint32_t)sizeof(*view);
  view->struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  return api.error_view(result->error, view) == OPENDAL_MBT_STATUS_OK;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_new(
    moonbit_string_t scheme, moonbit_string_t *keys,
    moonbit_string_t *values) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t scheme_utf8 = {0};
  owned_utf8_t *config_text = NULL;
  opendal_mbt_kv_v1_t *config = NULL;
  int32_t key_count;
  int32_t value_count;
  utf16_result_t conversion;
  result->status = load_api(&api, false);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (keys == NULL || values == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                           "InvalidArgument", "configuration arrays are NULL");
    return result;
  }
  key_count = Moonbit_array_length(keys);
  value_count = Moonbit_array_length(values);
  if (key_count < 0 || value_count != key_count) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                           "InvalidArgument",
                           "configuration key/value lengths differ");
    return result;
  }
  conversion = utf16_to_utf8(scheme, &scheme_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "scheme contains invalid UTF-16"
                                    : "unable to allocate UTF-8 scheme");
    return result;
  }
  if (key_count != 0) {
    size_t count = (size_t)(uint32_t)key_count;
    if (count > SIZE_MAX / (2 * sizeof(*config_text)) ||
        count > SIZE_MAX / sizeof(*config)) {
      result_set_local_error(result, OPENDAL_MBT_ERROR_UNEXPECTED,
                             "Unexpected", "configuration is too large");
      owned_utf8_free(&scheme_utf8);
      return result;
    }
    config_text = (owned_utf8_t *)calloc(count * 2, sizeof(*config_text));
    config = (opendal_mbt_kv_v1_t *)calloc(count, sizeof(*config));
    if (config_text == NULL || config == NULL) {
      result_set_local_error(result, OPENDAL_MBT_ERROR_UNEXPECTED,
                             "Unexpected",
                             "unable to allocate configuration conversion");
      free(config_text);
      free(config);
      owned_utf8_free(&scheme_utf8);
      return result;
    }
    for (int32_t i = 0; i < key_count; ++i) {
      utf16_result_t key_result =
          utf16_to_utf8(keys[i], &config_text[(size_t)i * 2]);
      utf16_result_t value_result =
          utf16_to_utf8(values[i], &config_text[(size_t)i * 2 + 1]);
      if (key_result != UTF16_OK || value_result != UTF16_OK) {
        result_set_local_error(
            result,
            key_result == UTF16_INVALID || value_result == UTF16_INVALID
                ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                : OPENDAL_MBT_ERROR_UNEXPECTED,
            key_result == UTF16_INVALID || value_result == UTF16_INVALID
                ? "InvalidArgument"
                : "Unexpected",
            key_result == UTF16_INVALID || value_result == UTF16_INVALID
                ? "configuration contains invalid UTF-16"
                : "unable to allocate UTF-8 configuration");
        for (int32_t j = 0; j <= i; ++j) {
          owned_utf8_free(&config_text[(size_t)j * 2]);
          owned_utf8_free(&config_text[(size_t)j * 2 + 1]);
        }
        free(config_text);
        free(config);
        owned_utf8_free(&scheme_utf8);
        return result;
      }
      config[i].key = owned_utf8_view(&config_text[(size_t)i * 2]);
      config[i].value = owned_utf8_view(&config_text[(size_t)i * 2 + 1]);
    }
  }
  {
    opendal_mbt_bytes_view_v1_t scheme_view = owned_utf8_view(&scheme_utf8);
    result->status = api.operator_new(
        &scheme_view, config, (uint64_t)(uint32_t)key_count, &result->operator_,
        &result->info, &result->error);
  }
  for (int32_t i = 0; i < key_count; ++i) {
    owned_utf8_free(&config_text[(size_t)i * 2]);
    owned_utf8_free(&config_text[(size_t)i * 2 + 1]);
  }
  free(config_text);
  free(config);
  owned_utf8_free(&scheme_utf8);
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      !validate_operator_info(result->info)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_s3(
    moonbit_string_t bucket, moonbit_string_t region, int32_t has_root,
    moonbit_string_t root, int32_t has_endpoint, moonbit_string_t endpoint,
    uint32_t auth_kind, uint32_t source_kind,
    int32_t disable_ec2_metadata, moonbit_string_t access_key_id,
    moonbit_string_t secret_access_key, int32_t has_session_token,
    moonbit_string_t session_token, moonbit_string_t role_arn,
    int32_t has_external_id, moonbit_string_t external_id,
    int32_t has_role_session_name, moonbit_string_t role_session_name,
    int32_t has_duration_seconds, uint32_t duration_seconds,
    int32_t virtual_host_style) {
  enum {
    S3_TEXT_BUCKET = 0,
    S3_TEXT_REGION,
    S3_TEXT_ROOT,
    S3_TEXT_ENDPOINT,
    S3_TEXT_ACCESS_KEY_ID,
    S3_TEXT_SECRET_ACCESS_KEY,
    S3_TEXT_SESSION_TOKEN,
    S3_TEXT_ROLE_ARN,
    S3_TEXT_EXTERNAL_ID,
    S3_TEXT_ROLE_SESSION_NAME,
    S3_TEXT_COUNT,
  };
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  opendal_mbt_s3_options_v1_t options;
  owned_utf8_t text[S3_TEXT_COUNT] = {{0}};
  bool uses_static_credentials = false;
  bool uses_assume_role = false;

  if ((has_root != 0 && has_root != 1) ||
      (has_endpoint != 0 && has_endpoint != 1) ||
      (disable_ec2_metadata != 0 && disable_ec2_metadata != 1) ||
      (has_session_token != 0 && has_session_token != 1) ||
      (has_external_id != 0 && has_external_id != 1) ||
      (has_role_session_name != 0 && has_role_session_name != 1) ||
      (has_duration_seconds != 0 && has_duration_seconds != 1) ||
      (virtual_host_style != 0 && virtual_host_style != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return result;
  }

  result->status = load_s3_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }

  switch (auth_kind) {
  case OPENDAL_MBT_S3_AUTH_DEFAULT_CHAIN:
    if (source_kind != OPENDAL_MBT_S3_SOURCE_DEFAULT_CHAIN ||
        has_session_token != 0 || has_external_id != 0 ||
        has_role_session_name != 0 || has_duration_seconds != 0 ||
        duration_seconds != 0) {
      result_set_local_error(
          result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT, "InvalidArgument",
          "invalid fields for default-chain S3 authentication");
      return result;
    }
    break;
  case OPENDAL_MBT_S3_AUTH_STATIC:
    if (source_kind != OPENDAL_MBT_S3_SOURCE_STATIC ||
        disable_ec2_metadata != 0 || has_external_id != 0 ||
        has_role_session_name != 0 || has_duration_seconds != 0 ||
        duration_seconds != 0) {
      result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                             "InvalidArgument",
                             "invalid fields for static S3 authentication");
      return result;
    }
    uses_static_credentials = true;
    break;
  case OPENDAL_MBT_S3_AUTH_UNSIGNED:
    if (source_kind != OPENDAL_MBT_S3_SOURCE_DEFAULT_CHAIN ||
        disable_ec2_metadata != 0 || has_session_token != 0 ||
        has_external_id != 0 || has_role_session_name != 0 ||
        has_duration_seconds != 0 || duration_seconds != 0) {
      result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                             "InvalidArgument",
                             "invalid fields for unsigned S3 authentication");
      return result;
    }
    break;
  case OPENDAL_MBT_S3_AUTH_ASSUME_ROLE:
    uses_assume_role = true;
    if (source_kind == OPENDAL_MBT_S3_SOURCE_STATIC) {
      if (disable_ec2_metadata != 0) {
        result_set_local_error(
            result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT, "InvalidArgument",
            "invalid fields for static-source S3 assume-role authentication");
        return result;
      }
      uses_static_credentials = true;
    } else if (source_kind == OPENDAL_MBT_S3_SOURCE_DEFAULT_CHAIN) {
      if (has_session_token != 0) {
        result_set_local_error(
            result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT, "InvalidArgument",
            "invalid fields for default-source S3 assume-role authentication");
        return result;
      }
    } else {
      result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                             "InvalidArgument",
                             "unsupported S3 credential source kind");
      return result;
    }
    if (has_duration_seconds == 0 && duration_seconds != 0) {
      result_set_local_error(
          result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT, "InvalidArgument",
          "S3 assume-role duration requires an explicit presence tag");
      return result;
    }
    break;
  default:
    result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                           "InvalidArgument",
                           "unsupported S3 authentication kind");
    return result;
  }

  if (!convert_optional_utf8(result, true, bucket,
                             "S3 bucket contains invalid UTF-16",
                             "unable to allocate UTF-8 S3 bucket",
                             &text[S3_TEXT_BUCKET]) ||
      !convert_optional_utf8(result, true, region,
                             "S3 region contains invalid UTF-16",
                             "unable to allocate UTF-8 S3 region",
                             &text[S3_TEXT_REGION]) ||
      !convert_optional_utf8(result, has_root != 0, root,
                             "S3 root contains invalid UTF-16",
                             "unable to allocate UTF-8 S3 root",
                             &text[S3_TEXT_ROOT]) ||
      !convert_optional_utf8(result, has_endpoint != 0, endpoint,
                             "S3 endpoint contains invalid UTF-16",
                             "unable to allocate UTF-8 S3 endpoint",
                             &text[S3_TEXT_ENDPOINT]) ||
      !convert_optional_utf8(result, uses_static_credentials, access_key_id,
                             "S3 access_key_id contains invalid UTF-16",
                             "unable to allocate UTF-8 S3 access_key_id",
                             &text[S3_TEXT_ACCESS_KEY_ID]) ||
      !convert_optional_utf8(result, uses_static_credentials,
                             secret_access_key,
                             "S3 secret_access_key contains invalid UTF-16",
                             "unable to allocate UTF-8 S3 secret_access_key",
                             &text[S3_TEXT_SECRET_ACCESS_KEY]) ||
      !convert_optional_utf8(result,
                             uses_static_credentials && has_session_token != 0,
                             session_token,
                             "S3 session_token contains invalid UTF-16",
                             "unable to allocate UTF-8 S3 session_token",
                             &text[S3_TEXT_SESSION_TOKEN]) ||
      !convert_optional_utf8(result, uses_assume_role, role_arn,
                             "S3 role_arn contains invalid UTF-16",
                             "unable to allocate UTF-8 S3 role_arn",
                             &text[S3_TEXT_ROLE_ARN]) ||
      !convert_optional_utf8(result,
                             uses_assume_role && has_external_id != 0,
                             external_id,
                             "S3 external_id contains invalid UTF-16",
                             "unable to allocate UTF-8 S3 external_id",
                             &text[S3_TEXT_EXTERNAL_ID]) ||
      !convert_optional_utf8(result,
                             uses_assume_role && has_role_session_name != 0,
                             role_session_name,
                             "S3 role_session_name contains invalid UTF-16",
                             "unable to allocate UTF-8 S3 role_session_name",
                             &text[S3_TEXT_ROLE_SESSION_NAME])) {
    goto cleanup;
  }

  memset(&options, 0, sizeof(options));
  options.struct_size = (uint32_t)sizeof(options);
  options.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options.auth_kind = auth_kind;
  options.source_kind = source_kind;
  options.bucket = owned_utf8_view(&text[S3_TEXT_BUCKET]);
  options.region = owned_utf8_view(&text[S3_TEXT_REGION]);
  if (has_root != 0) {
    options.present_bits |= OPENDAL_MBT_S3_ROOT_PRESENT;
    options.root = owned_utf8_view(&text[S3_TEXT_ROOT]);
  }
  if (has_endpoint != 0) {
    options.present_bits |= OPENDAL_MBT_S3_ENDPOINT_PRESENT;
    options.endpoint = owned_utf8_view(&text[S3_TEXT_ENDPOINT]);
  }
  if (uses_static_credentials) {
    options.access_key_id = owned_utf8_view(&text[S3_TEXT_ACCESS_KEY_ID]);
    options.secret_access_key =
        owned_utf8_view(&text[S3_TEXT_SECRET_ACCESS_KEY]);
    if (has_session_token != 0) {
      options.present_bits |= OPENDAL_MBT_S3_SESSION_TOKEN_PRESENT;
      options.session_token = owned_utf8_view(&text[S3_TEXT_SESSION_TOKEN]);
    }
  }
  if (uses_assume_role) {
    options.role_arn = owned_utf8_view(&text[S3_TEXT_ROLE_ARN]);
    if (has_external_id != 0) {
      options.present_bits |= OPENDAL_MBT_S3_EXTERNAL_ID_PRESENT;
      options.external_id = owned_utf8_view(&text[S3_TEXT_EXTERNAL_ID]);
    }
    if (has_role_session_name != 0) {
      options.present_bits |= OPENDAL_MBT_S3_ROLE_SESSION_NAME_PRESENT;
      options.role_session_name =
          owned_utf8_view(&text[S3_TEXT_ROLE_SESSION_NAME]);
    }
    if (has_duration_seconds != 0) {
      options.present_bits |= OPENDAL_MBT_S3_ASSUME_ROLE_DURATION_PRESENT;
      options.assume_role_duration_seconds = duration_seconds;
    }
  }
  if (virtual_host_style != 0) {
    options.flags |= OPENDAL_MBT_S3_VIRTUAL_HOST_STYLE;
  }
  if (disable_ec2_metadata != 0) {
    options.flags |= OPENDAL_MBT_S3_DISABLE_EC2_METADATA;
  }

  result->status = api.operator_s3(&options, &result->operator_, &result->info,
                                   &result->error);

cleanup:
  for (int32_t i = 0; i < S3_TEXT_COUNT; ++i) {
    owned_utf8_free(&text[i]);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->error != NULL || result->operator_ == NULL ||
       !validate_operator_info(result->info))) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_operator_with_timeout(
    moonbit_opendal_operator_t *operator_, uint64_t operation_timeout_millis,
    uint64_t io_timeout_millis) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  result->status = load_layers_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  result->status = api.operator_with_timeout(
      operator_->operator_, operation_timeout_millis, io_timeout_millis,
      &result->operator_, &result->info, &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->operator_ == NULL || result->info == NULL ||
       result->error != NULL || !validate_operator_info(result->info))) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_operator_with_retry(
    moonbit_opendal_operator_t *operator_, uint32_t max_retries,
    uint64_t min_delay_millis, uint64_t max_delay_millis, int32_t jitter) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  if (jitter != 0 && jitter != 1) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return result;
  }
  result->status = load_layers_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  result->status = api.operator_with_retry(
      operator_->operator_, max_retries, min_delay_millis, max_delay_millis,
      jitter == 0 ? OPENDAL_MBT_FALSE : OPENDAL_MBT_TRUE,
      &result->operator_, &result->info, &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->operator_ == NULL || result->info == NULL ||
       result->error != NULL || !validate_operator_info(result->info))) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_operator_with_concurrency_limit(
    moonbit_opendal_operator_t *operator_, uint64_t operation_limit,
    int32_t has_http_request_limit, uint64_t http_request_limit) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  if (has_http_request_limit != 0 && has_http_request_limit != 1) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return result;
  }
  result->status = load_concurrency_limit_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  result->status = api.operator_with_concurrency_limit(
      operator_->operator_, operation_limit,
      has_http_request_limit == 0 ? OPENDAL_MBT_FALSE : OPENDAL_MBT_TRUE,
      http_request_limit, &result->operator_, &result->info, &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->operator_ == NULL || result->info == NULL ||
       result->error != NULL || !validate_operator_info(result->info))) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_check(
    moonbit_opendal_operator_t *operator_) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  result->status = load_api(&api, true);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  result->status =
      api.operator_check(operator_->operator_, &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK && result->error != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_exists(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  utf16_result_t conversion;
  opendal_mbt_bool_t exists = OPENDAL_MBT_FALSE;
  result->status = load_api(&api, true);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    return result;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_exists(operator_->operator_, &path_view,
                                         &exists, &result->error);
  }
  owned_utf8_free(&path_utf8);
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if ((exists != OPENDAL_MBT_FALSE && exists != OPENDAL_MBT_TRUE) ||
        result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    } else {
      result->bool_value = exists;
      result->has_bool = true;
    }
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_stat(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    int32_t has_version, moonbit_string_t version, int32_t has_if_match,
    moonbit_string_t if_match, int32_t has_if_none_match,
    moonbit_string_t if_none_match) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[3] = {{0}};
  opendal_mbt_stat_options_v1_t options;
  utf16_result_t conversion;
  if ((has_version != 0 && has_version != 1) ||
      (has_if_match != 0 && has_if_match != 1) ||
      (has_if_none_match != 0 && has_if_none_match != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return result;
  }
  result->status = load_api(&api, true);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup;
  }
  if (!convert_optional_utf8(
          result, has_version != 0, version,
          "stat version contains invalid UTF-16",
          "unable to allocate UTF-8 stat version", &option_utf8[0]) ||
      !convert_optional_utf8(
          result, has_if_match != 0, if_match,
          "stat if_match contains invalid UTF-16",
          "unable to allocate UTF-8 stat if_match", &option_utf8[1]) ||
      !convert_optional_utf8(
          result, has_if_none_match != 0, if_none_match,
          "stat if_none_match contains invalid UTF-16",
          "unable to allocate UTF-8 stat if_none_match", &option_utf8[2])) {
    goto cleanup;
  }
  memset(&options, 0, sizeof(options));
  options.struct_size = (uint32_t)sizeof(options);
  options.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  if (has_version != 0) {
    options.present_bits |= OPENDAL_MBT_STAT_VERSION_PRESENT;
    options.version = owned_utf8_view(&option_utf8[0]);
  }
  if (has_if_match != 0) {
    options.present_bits |= OPENDAL_MBT_STAT_IF_MATCH_PRESENT;
    options.if_match = owned_utf8_view(&option_utf8[1]);
  }
  if (has_if_none_match != 0) {
    options.present_bits |= OPENDAL_MBT_STAT_IF_NONE_MATCH_PRESENT;
    options.if_none_match = owned_utf8_view(&option_utf8[2]);
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_stat(
        operator_->operator_, &path_view, &options, &result->metadata,
        &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->metadata == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 3; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_read(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    uint32_t range_kind, uint64_t range_offset, uint64_t range_length,
    int32_t has_version, moonbit_string_t version, int32_t has_if_match,
    moonbit_string_t if_match, int32_t has_if_none_match,
    moonbit_string_t if_none_match) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[3] = {{0}};
  opendal_mbt_read_options_v1_t options;
  utf16_result_t conversion;
  if ((has_version != 0 && has_version != 1) ||
      (has_if_match != 0 && has_if_match != 1) ||
      (has_if_none_match != 0 && has_if_none_match != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return result;
  }
  result->status = load_api(&api, true);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup;
  }
  if (!convert_optional_utf8(
          result, has_version != 0, version,
          "read version contains invalid UTF-16",
          "unable to allocate UTF-8 read version", &option_utf8[0]) ||
      !convert_optional_utf8(
          result, has_if_match != 0, if_match,
          "read if_match contains invalid UTF-16",
          "unable to allocate UTF-8 read if_match", &option_utf8[1]) ||
      !convert_optional_utf8(
          result, has_if_none_match != 0, if_none_match,
          "read if_none_match contains invalid UTF-16",
          "unable to allocate UTF-8 read if_none_match", &option_utf8[2])) {
    goto cleanup;
  }
  memset(&options, 0, sizeof(options));
  options.struct_size = (uint32_t)sizeof(options);
  options.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options.range.struct_size = (uint32_t)sizeof(options.range);
  options.range.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options.range.kind = range_kind;
  options.range.offset = range_offset;
  options.range.length = range_length;
  if (has_version != 0) {
    options.present_bits |= OPENDAL_MBT_READ_VERSION_PRESENT;
    options.version = owned_utf8_view(&option_utf8[0]);
  }
  if (has_if_match != 0) {
    options.present_bits |= OPENDAL_MBT_READ_IF_MATCH_PRESENT;
    options.if_match = owned_utf8_view(&option_utf8[1]);
  }
  if (has_if_none_match != 0) {
    options.present_bits |= OPENDAL_MBT_READ_IF_NONE_MATCH_PRESENT;
    options.if_none_match = owned_utf8_view(&option_utf8[2]);
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_read(
        operator_->operator_, &path_view, &options, (uint64_t)INT32_MAX,
        &result->buffer, &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->buffer == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 3; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_reader(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    int32_t has_version, moonbit_string_t version, int32_t has_if_match,
    moonbit_string_t if_match, int32_t has_if_none_match,
    moonbit_string_t if_none_match) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[3] = {{0}};
  opendal_mbt_reader_options_v1_t options;
  utf16_result_t conversion;
  if ((has_version != 0 && has_version != 1) ||
      (has_if_match != 0 && has_if_match != 1) ||
      (has_if_none_match != 0 && has_if_none_match != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return result;
  }
  result->status = load_reader_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup_reader;
  }
  if (!convert_optional_utf8(
          result, has_version != 0, version,
          "reader version contains invalid UTF-16",
          "unable to allocate UTF-8 reader version", &option_utf8[0]) ||
      !convert_optional_utf8(
          result, has_if_match != 0, if_match,
          "reader if_match contains invalid UTF-16",
          "unable to allocate UTF-8 reader if_match", &option_utf8[1]) ||
      !convert_optional_utf8(
          result, has_if_none_match != 0, if_none_match,
          "reader if_none_match contains invalid UTF-16",
          "unable to allocate UTF-8 reader if_none_match", &option_utf8[2])) {
    goto cleanup_reader;
  }
  memset(&options, 0, sizeof(options));
  options.struct_size = (uint32_t)sizeof(options);
  options.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  if (has_version != 0) {
    options.present_bits |= OPENDAL_MBT_READER_VERSION_PRESENT;
    options.version = owned_utf8_view(&option_utf8[0]);
  }
  if (has_if_match != 0) {
    options.present_bits |= OPENDAL_MBT_READER_IF_MATCH_PRESENT;
    options.if_match = owned_utf8_view(&option_utf8[1]);
  }
  if (has_if_none_match != 0) {
    options.present_bits |= OPENDAL_MBT_READER_IF_NONE_MATCH_PRESENT;
    options.if_none_match = owned_utf8_view(&option_utf8[2]);
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_reader(operator_->operator_, &path_view,
                                         &options, &result->reader,
                                         &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->reader == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_reader:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 3; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_reader_read(
    moonbit_opendal_reader_t *reader, uint32_t range_kind,
    uint64_t range_offset, uint64_t range_length) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  opendal_mbt_byte_range_v1_t range;
  result->status = load_reader_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (reader == NULL || reader->reader == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "reader is closed");
    return result;
  }
  memset(&range, 0, sizeof(range));
  range.struct_size = (uint32_t)sizeof(range);
  range.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  range.kind = range_kind;
  range.offset = range_offset;
  range.length = range_length;
  result->status = api.reader_read(reader->reader, &range,
                                   (uint64_t)INT32_MAX, &result->buffer,
                                   &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->buffer == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT void
moonbit_opendal_reader_close(moonbit_opendal_reader_t *reader) {
  opendal_mbt_api_v1_t api;
  if (reader != NULL && reader->reader != NULL &&
      load_reader_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.reader_close(reader->reader);
  }
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_operator_read_stream(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    uint32_t range_kind, uint64_t range_offset, uint64_t range_length,
    int32_t chunk_size, int32_t has_version, moonbit_string_t version,
    int32_t has_if_match, moonbit_string_t if_match,
    int32_t has_if_none_match, moonbit_string_t if_none_match) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[3] = {{0}};
  opendal_mbt_read_stream_options_v1_t options;
  utf16_result_t conversion;
  if ((has_version != 0 && has_version != 1) ||
      (has_if_match != 0 && has_if_match != 1) ||
      (has_if_none_match != 0 && has_if_none_match != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return result;
  }
  if (chunk_size <= 0) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                           "InvalidArgument",
                           "read stream chunk_size must be positive");
    return result;
  }
  result->status = load_read_stream_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if ((uint64_t)(uint32_t)chunk_size > api.max_output_bytes) {
    result_set_local_error(
        result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT, "InvalidArgument",
        "read stream chunk_size exceeds the negotiated native output limit");
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup_read_stream;
  }
  if (!convert_optional_utf8(
          result, has_version != 0, version,
          "read stream version contains invalid UTF-16",
          "unable to allocate UTF-8 read stream version", &option_utf8[0]) ||
      !convert_optional_utf8(
          result, has_if_match != 0, if_match,
          "read stream if_match contains invalid UTF-16",
          "unable to allocate UTF-8 read stream if_match", &option_utf8[1]) ||
      !convert_optional_utf8(
          result, has_if_none_match != 0, if_none_match,
          "read stream if_none_match contains invalid UTF-16",
          "unable to allocate UTF-8 read stream if_none_match",
          &option_utf8[2])) {
    goto cleanup_read_stream;
  }
  memset(&options, 0, sizeof(options));
  options.struct_size = (uint32_t)sizeof(options);
  options.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options.range.struct_size = (uint32_t)sizeof(options.range);
  options.range.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options.range.kind = range_kind;
  options.range.offset = range_offset;
  options.range.length = range_length;
  options.chunk_size = (uint64_t)(uint32_t)chunk_size;
  if (has_version != 0) {
    options.present_bits |= OPENDAL_MBT_READ_STREAM_VERSION_PRESENT;
    options.version = owned_utf8_view(&option_utf8[0]);
  }
  if (has_if_match != 0) {
    options.present_bits |= OPENDAL_MBT_READ_STREAM_IF_MATCH_PRESENT;
    options.if_match = owned_utf8_view(&option_utf8[1]);
  }
  if (has_if_none_match != 0) {
    options.present_bits |= OPENDAL_MBT_READ_STREAM_IF_NONE_MATCH_PRESENT;
    options.if_none_match = owned_utf8_view(&option_utf8[2]);
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_read_stream(
        operator_->operator_, &path_view, &options, &result->read_stream,
        &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if (result->read_stream == NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->read_stream != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_read_stream:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 3; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_read_stream_next(moonbit_opendal_read_stream_t *stream) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  result->status = load_read_stream_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (stream == NULL || stream->stream == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "read stream is closed");
    return result;
  }
  result->status = api.read_stream_next(
      stream->stream, api.max_output_bytes, &result->buffer, &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if (result->buffer == NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->status == OPENDAL_MBT_STATUS_END) {
    if (result->buffer != NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->buffer != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT void moonbit_opendal_read_stream_close(
    moonbit_opendal_read_stream_t *stream) {
  opendal_mbt_api_v1_t api;
  if (stream != NULL && stream->stream != NULL &&
      load_read_stream_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.read_stream_close(stream->stream);
  }
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_write(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    moonbit_bytes_t data, int32_t append, int32_t has_content_type,
    moonbit_string_t content_type, int32_t has_content_disposition,
    moonbit_string_t content_disposition, int32_t has_content_encoding,
    moonbit_string_t content_encoding, int32_t has_cache_control,
    moonbit_string_t cache_control, int32_t has_if_match,
    moonbit_string_t if_match, int32_t has_if_none_match,
    moonbit_string_t if_none_match) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[6] = {{0}};
  opendal_mbt_write_options_v1_t options;
  utf16_result_t conversion;
  int32_t data_len;
  result->status = load_api(&api, true);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  if (data == NULL || (data_len = Moonbit_array_length(data)) < 0) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                           "InvalidArgument", "data is invalid");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup_write;
  }
  if (!prepare_write_options(
          result, append, has_content_type, content_type,
          has_content_disposition, content_disposition, has_content_encoding,
          content_encoding, has_cache_control, cache_control, has_if_match,
          if_match, has_if_none_match, if_none_match, option_utf8, &options)) {
    goto cleanup_write;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    opendal_mbt_bytes_view_v1_t data_view = {
        .data = data,
        .len = (uint64_t)(uint32_t)data_len,
    };
    result->status = api.operator_write(
        operator_->operator_, &path_view, &data_view, &options,
        &result->metadata, &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->metadata == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_write:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 6; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_operator_presign_read(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    uint64_t expires_in_seconds, uint32_t range_kind, uint64_t range_offset,
    uint64_t range_length, int32_t has_version, moonbit_string_t version,
    int32_t has_if_match, moonbit_string_t if_match,
    int32_t has_if_none_match, moonbit_string_t if_none_match) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[3] = {{0}};
  opendal_mbt_read_options_v1_t options;
  utf16_result_t conversion;
  result->status = load_presign_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup_presign_read;
  }
  if (!prepare_read_options(
          result, range_kind, range_offset, range_length, has_version,
          version, has_if_match, if_match, has_if_none_match, if_none_match,
          option_utf8, &options)) {
    goto cleanup_presign_read;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_presign_read(
        operator_->operator_, &path_view, &options, expires_in_seconds,
        &result->presigned_request, &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if (result->presigned_request == NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->presigned_request != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_presign_read:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 3; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_operator_presign_write(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    uint64_t expires_in_seconds, int32_t append, int32_t has_content_type,
    moonbit_string_t content_type, int32_t has_content_disposition,
    moonbit_string_t content_disposition, int32_t has_content_encoding,
    moonbit_string_t content_encoding, int32_t has_cache_control,
    moonbit_string_t cache_control, int32_t has_if_match,
    moonbit_string_t if_match, int32_t has_if_none_match,
    moonbit_string_t if_none_match) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[6] = {{0}};
  opendal_mbt_write_options_v1_t options;
  utf16_result_t conversion;
  result->status = load_presign_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup_presign_write;
  }
  if (!prepare_write_options(
          result, append, has_content_type, content_type,
          has_content_disposition, content_disposition, has_content_encoding,
          content_encoding, has_cache_control, cache_control, has_if_match,
          if_match, has_if_none_match, if_none_match, option_utf8, &options)) {
    goto cleanup_presign_write;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_presign_write(
        operator_->operator_, &path_view, &options, expires_in_seconds,
        &result->presigned_request, &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if (result->presigned_request == NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->presigned_request != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_presign_write:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 6; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_operator_presign_stat(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    uint64_t expires_in_seconds, int32_t has_version,
    moonbit_string_t version, int32_t has_if_match, moonbit_string_t if_match,
    int32_t has_if_none_match, moonbit_string_t if_none_match) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[3] = {{0}};
  opendal_mbt_stat_options_v1_t options;
  utf16_result_t conversion;
  result->status = load_presign_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup_presign_stat;
  }
  if (!prepare_stat_options(result, has_version, version, has_if_match,
                            if_match, has_if_none_match, if_none_match,
                            option_utf8, &options)) {
    goto cleanup_presign_stat;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_presign_stat(
        operator_->operator_, &path_view, &options, expires_in_seconds,
        &result->presigned_request, &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if (result->presigned_request == NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->presigned_request != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_presign_stat:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 3; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_writer(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    int32_t append, int32_t has_content_type, moonbit_string_t content_type,
    int32_t has_content_disposition, moonbit_string_t content_disposition,
    int32_t has_content_encoding, moonbit_string_t content_encoding,
    int32_t has_cache_control, moonbit_string_t cache_control,
    int32_t has_if_match, moonbit_string_t if_match,
    int32_t has_if_none_match, moonbit_string_t if_none_match) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[6] = {{0}};
  opendal_mbt_write_options_v1_t options;
  utf16_result_t conversion;
  result->status = load_writer_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup_writer;
  }
  if (!prepare_write_options(
          result, append, has_content_type, content_type,
          has_content_disposition, content_disposition, has_content_encoding,
          content_encoding, has_cache_control, cache_control, has_if_match,
          if_match, has_if_none_match, if_none_match, option_utf8, &options)) {
    goto cleanup_writer;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_writer(operator_->operator_, &path_view,
                                         &options, &result->writer,
                                         &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->writer == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_writer:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 6; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_writer_write(
    moonbit_opendal_writer_t *writer, moonbit_bytes_t data) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  int32_t data_len;
  result->status = load_writer_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (writer == NULL || writer->writer == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "writer is closed");
    return result;
  }
  if (data == NULL || (data_len = Moonbit_array_length(data)) < 0) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                           "InvalidArgument", "data is invalid");
    return result;
  }
  {
    opendal_mbt_bytes_view_v1_t data_view = {
        .data = data,
        .len = (uint64_t)(uint32_t)data_len,
    };
    result->status =
        api.writer_write(writer->writer, &data_view, &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK && result->error != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_writer_close(moonbit_opendal_writer_t *writer) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  result->status = load_writer_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (writer == NULL || writer->writer == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "writer is closed");
    return result;
  }
  result->status = api.writer_close(writer->writer, &result->metadata,
                                    &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->metadata == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_writer_abort(
    moonbit_opendal_writer_t *writer) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  result->status = load_writer_abort_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (writer == NULL || writer->writer == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "writer is closed");
    return result;
  }
  result->status = api.writer_abort(writer->writer, &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK && result->error != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_async_operator_read_start(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    uint32_t range_kind, uint64_t range_offset, uint64_t range_length,
    int32_t has_version, moonbit_string_t version, int32_t has_if_match,
    moonbit_string_t if_match, int32_t has_if_none_match,
    moonbit_string_t if_none_match, int32_t completion_fd) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[3] = {{0}};
  opendal_mbt_read_options_v1_t options;
  utf16_result_t conversion;
  result->status = load_async_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup_async_read;
  }
  if (!prepare_async_read_options(
          result, range_kind, range_offset, range_length, has_version, version,
          has_if_match, if_match, has_if_none_match, if_none_match,
          option_utf8, &options)) {
    goto cleanup_async_read;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.async_operator_read_start(
        operator_->operator_, &path_view, &options, api.max_output_bytes,
        completion_fd, &result->async_task, &result->error);
    result->async_kind = MOONBIT_OPENDAL_ASYNC_BUFFER;
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->async_task == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_async_read:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 3; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_async_operator_read_stream_start(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    uint32_t range_kind, uint64_t range_offset, uint64_t range_length,
    int32_t chunk_size, int32_t has_version, moonbit_string_t version,
    int32_t has_if_match, moonbit_string_t if_match,
    int32_t has_if_none_match, moonbit_string_t if_none_match,
    int32_t completion_fd) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[3] = {{0}};
  opendal_mbt_read_stream_options_v1_t options;
  utf16_result_t conversion;
  result->status = load_async_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  if ((uint64_t)(uint32_t)chunk_size > api.max_output_bytes) {
    result_set_local_error(
        result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT, "InvalidArgument",
        "async read stream chunk_size exceeds the native output limit");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup_async_read_stream;
  }
  if (!prepare_async_read_stream_options(
          result, range_kind, range_offset, range_length, chunk_size,
          has_version, version, has_if_match, if_match, has_if_none_match,
          if_none_match, option_utf8, &options)) {
    goto cleanup_async_read_stream;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.async_operator_read_stream_start(
        operator_->operator_, &path_view, &options, completion_fd,
        &result->async_task, &result->error);
    result->async_kind = MOONBIT_OPENDAL_ASYNC_READ_STREAM;
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->async_task == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_async_read_stream:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 3; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_async_read_stream_next_start(
    moonbit_opendal_async_read_stream_t *stream, int32_t completion_fd) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  result->status = load_async_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (stream == NULL || stream->stream == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "async read stream is closed");
    return result;
  }
  result->status = api.async_read_stream_next_start(
      stream->stream, api.max_output_bytes, completion_fd,
      &result->async_task, &result->error);
  result->async_kind = MOONBIT_OPENDAL_ASYNC_BUFFER;
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->async_task == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT void moonbit_opendal_async_read_stream_close(
    moonbit_opendal_async_read_stream_t *stream) {
  opendal_mbt_api_v1_t api;
  if (stream != NULL && stream->stream != NULL &&
      load_async_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.async_read_stream_close(stream->stream);
  }
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_async_operator_writer_start(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    int32_t append, int32_t has_content_type, moonbit_string_t content_type,
    int32_t has_content_disposition, moonbit_string_t content_disposition,
    int32_t has_content_encoding, moonbit_string_t content_encoding,
    int32_t has_cache_control, moonbit_string_t cache_control,
    int32_t has_if_match, moonbit_string_t if_match,
    int32_t has_if_none_match, moonbit_string_t if_none_match,
    int32_t completion_fd) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t option_utf8[6] = {{0}};
  opendal_mbt_write_options_v1_t options;
  utf16_result_t conversion;
  result->status = load_async_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup_async_writer;
  }
  if (!prepare_write_options(
          result, append, has_content_type, content_type,
          has_content_disposition, content_disposition, has_content_encoding,
          content_encoding, has_cache_control, cache_control, has_if_match,
          if_match, has_if_none_match, if_none_match, option_utf8, &options)) {
    goto cleanup_async_writer;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.async_operator_writer_start(
        operator_->operator_, &path_view, &options, completion_fd,
        &result->async_task, &result->error);
    result->async_kind = MOONBIT_OPENDAL_ASYNC_WRITER;
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->async_task == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_async_writer:
  owned_utf8_free(&path_utf8);
  for (size_t i = 0; i < 6; ++i) {
    owned_utf8_free(&option_utf8[i]);
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_async_writer_write_start(
    moonbit_opendal_async_writer_t *writer, moonbit_bytes_t data,
    int32_t completion_fd) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  int32_t data_len;
  result->status = load_async_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (writer == NULL || writer->writer == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "async writer is closed");
    return result;
  }
  if (data == NULL || (data_len = Moonbit_array_length(data)) < 0) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                           "InvalidArgument", "data is invalid");
    return result;
  }
  {
    opendal_mbt_bytes_view_v1_t data_view = {
        .data = data,
        .len = (uint64_t)(uint32_t)data_len,
    };
    result->status = api.async_writer_write_start(
        writer->writer, &data_view, completion_fd, &result->async_task,
        &result->error);
    result->async_kind = MOONBIT_OPENDAL_ASYNC_UNIT;
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->async_task == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_async_writer_finish_start(
    moonbit_opendal_async_writer_t *writer, int32_t completion_fd) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  result->status = load_async_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (writer == NULL || writer->writer == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "async writer is closed");
    return result;
  }
  result->status = api.async_writer_finish_start(
      writer->writer, completion_fd, &result->async_task, &result->error);
  result->async_kind = MOONBIT_OPENDAL_ASYNC_METADATA;
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->async_task == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_async_writer_abort_start(
    moonbit_opendal_async_writer_t *writer, int32_t completion_fd) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  result->status = load_async_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (writer == NULL || writer->writer == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "async writer is closed");
    return result;
  }
  result->status = api.async_writer_abort_start(
      writer->writer, completion_fd, &result->async_task, &result->error);
  result->async_kind = MOONBIT_OPENDAL_ASYNC_UNIT;
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->async_task == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_async_task_t *
moonbit_opendal_result_take_async_task(moonbit_opendal_result_t *result) {
  moonbit_opendal_async_task_t *task = async_task_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->async_task == NULL || result->async_kind == 0) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return task;
  }
  task->task = result->async_task;
  task->kind = result->async_kind;
  result->async_task = NULL;
  result->async_kind = 0;
  return task;
}

MOONBIT_FFI_EXPORT moonbit_opendal_async_read_stream_t *
moonbit_opendal_result_take_async_read_stream(
    moonbit_opendal_result_t *result) {
  moonbit_opendal_async_read_stream_t *stream =
      async_read_stream_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->async_read_stream == NULL) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return stream;
  }
  stream->stream = result->async_read_stream;
  result->async_read_stream = NULL;
  return stream;
}

MOONBIT_FFI_EXPORT moonbit_opendal_async_writer_t *
moonbit_opendal_result_take_async_writer(moonbit_opendal_result_t *result) {
  moonbit_opendal_async_writer_t *writer = async_writer_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->async_writer == NULL) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return writer;
  }
  writer->writer = result->async_writer;
  result->async_writer = NULL;
  return writer;
}

MOONBIT_FFI_EXPORT void moonbit_opendal_async_task_cancel(
    moonbit_opendal_async_task_t *task) {
  opendal_mbt_api_v1_t api;
  if (task != NULL && task->task != NULL &&
      load_async_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.async_task_cancel(task->task);
  }
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_async_task_take_result(moonbit_opendal_async_task_t *task) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  result->status = load_async_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (task == NULL || task->task == NULL || task->kind == 0) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "async task is closed");
    return result;
  }
  switch (task->kind) {
  case MOONBIT_OPENDAL_ASYNC_BUFFER:
    result->status = api.async_task_take_buffer(
        task->task, &result->buffer, &result->error);
    if (result->status == OPENDAL_MBT_STATUS_OK) {
      if (result->buffer == NULL || result->error != NULL) {
        result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
      }
    } else if (result->status == OPENDAL_MBT_STATUS_END) {
      if (result->buffer != NULL || result->error != NULL) {
        result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
      }
    } else if (result->buffer != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    break;
  case MOONBIT_OPENDAL_ASYNC_METADATA:
    result->status = api.async_task_take_metadata(
        task->task, &result->metadata, &result->error);
    if (result->status == OPENDAL_MBT_STATUS_OK &&
        (result->metadata == NULL || result->error != NULL)) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    break;
  case MOONBIT_OPENDAL_ASYNC_READ_STREAM:
    result->status = api.async_task_take_read_stream(
        task->task, &result->async_read_stream, &result->error);
    if (result->status == OPENDAL_MBT_STATUS_OK &&
        (result->async_read_stream == NULL || result->error != NULL)) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    break;
  case MOONBIT_OPENDAL_ASYNC_WRITER:
    result->status = api.async_task_take_writer(
        task->task, &result->async_writer, &result->error);
    if (result->status == OPENDAL_MBT_STATUS_OK &&
        (result->async_writer == NULL || result->error != NULL)) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    break;
  case MOONBIT_OPENDAL_ASYNC_UNIT:
    result->status = api.async_task_take_unit(task->task, &result->error);
    if (result->status == OPENDAL_MBT_STATUS_OK && result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    break;
  default:
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    break;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_copy(
    moonbit_opendal_operator_t *operator_, moonbit_string_t source,
    moonbit_string_t destination) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t source_utf8 = {0};
  owned_utf8_t destination_utf8 = {0};
  utf16_result_t conversion;
  result->status = load_api(&api, true);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(source, &source_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "source path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 source path");
    goto cleanup_copy;
  }
  conversion = utf16_to_utf8(destination, &destination_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID
            ? "destination path contains invalid UTF-16"
            : "unable to allocate UTF-8 destination path");
    goto cleanup_copy;
  }
  {
    opendal_mbt_bytes_view_v1_t source_view = owned_utf8_view(&source_utf8);
    opendal_mbt_bytes_view_v1_t destination_view =
        owned_utf8_view(&destination_utf8);
    result->status = api.operator_copy(
        operator_->operator_, &source_view, &destination_view,
        &result->metadata, &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK &&
      (result->metadata == NULL || result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_copy:
  owned_utf8_free(&destination_utf8);
  owned_utf8_free(&source_utf8);
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_rename(
    moonbit_opendal_operator_t *operator_, moonbit_string_t source,
    moonbit_string_t destination) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t source_utf8 = {0};
  owned_utf8_t destination_utf8 = {0};
  utf16_result_t conversion;
  result->status = load_api(&api, true);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(source, &source_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "source path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 source path");
    goto cleanup_rename;
  }
  conversion = utf16_to_utf8(destination, &destination_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID
            ? "destination path contains invalid UTF-16"
            : "unable to allocate UTF-8 destination path");
    goto cleanup_rename;
  }
  {
    opendal_mbt_bytes_view_v1_t source_view = owned_utf8_view(&source_utf8);
    opendal_mbt_bytes_view_v1_t destination_view =
        owned_utf8_view(&destination_utf8);
    result->status = api.operator_rename(operator_->operator_, &source_view,
                                         &destination_view, &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK && result->error != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_rename:
  owned_utf8_free(&destination_utf8);
  owned_utf8_free(&source_utf8);
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_operator_create_dir(moonbit_opendal_operator_t *operator_,
                                    moonbit_string_t path) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  utf16_result_t conversion;
  result->status = load_api(&api, true);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    return result;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_create_dir(operator_->operator_, &path_view,
                                             &result->error);
  }
  owned_utf8_free(&path_utf8);
  if (result->status == OPENDAL_MBT_STATUS_OK && result->error != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_delete(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    int32_t has_version, moonbit_string_t version, int32_t recursive) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t version_utf8 = {0};
  opendal_mbt_delete_options_v1_t options;
  utf16_result_t conversion;
  if ((has_version != 0 && has_version != 1) ||
      (recursive != 0 && recursive != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return result;
  }
  result->status = load_api(&api, true);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup;
  }
  if (!convert_optional_utf8(
          result, has_version != 0, version,
          "delete version contains invalid UTF-16",
          "unable to allocate UTF-8 delete version", &version_utf8)) {
    goto cleanup;
  }
  memset(&options, 0, sizeof(options));
  options.struct_size = (uint32_t)sizeof(options);
  options.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  if (has_version != 0) {
    options.present_bits = OPENDAL_MBT_DELETE_VERSION_PRESENT;
    options.version = owned_utf8_view(&version_utf8);
  }
  if (recursive != 0) {
    options.flags = OPENDAL_MBT_DELETE_RECURSIVE;
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_delete(operator_->operator_, &path_view,
                                         &options, &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK && result->error != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup:
  owned_utf8_free(&path_utf8);
  owned_utf8_free(&version_utf8);
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_operator_delete_many(moonbit_opendal_operator_t *operator_,
                                     moonbit_string_t *paths) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t *path_utf8 = NULL;
  opendal_mbt_bytes_view_v1_t *path_views = NULL;
  int32_t path_count;
  int32_t converted_count = 0;

  result->status = load_batch_delete_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  if (paths == NULL || (path_count = Moonbit_array_length(paths)) < 0) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_INVALID_ARGUMENT,
                           "InvalidArgument", "batch delete paths are invalid");
    return result;
  }
  if (path_count != 0) {
    size_t count = (size_t)(uint32_t)path_count;
    if (count > SIZE_MAX / sizeof(*path_utf8) ||
        count > SIZE_MAX / sizeof(*path_views)) {
      result_set_local_error(result, OPENDAL_MBT_ERROR_UNEXPECTED,
                             "Unexpected", "batch delete input is too large");
      return result;
    }
    path_utf8 = (owned_utf8_t *)calloc(count, sizeof(*path_utf8));
    path_views =
        (opendal_mbt_bytes_view_v1_t *)calloc(count, sizeof(*path_views));
    if (path_utf8 == NULL || path_views == NULL) {
      result_set_local_error(result, OPENDAL_MBT_ERROR_UNEXPECTED,
                             "Unexpected",
                             "unable to allocate batch delete conversion");
      goto cleanup_delete_many;
    }
    for (int32_t i = 0; i < path_count; ++i) {
      utf16_result_t conversion = utf16_to_utf8(paths[i], &path_utf8[i]);
      if (conversion != UTF16_OK) {
        result_set_local_error(
            result,
            conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                        : OPENDAL_MBT_ERROR_UNEXPECTED,
            conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
            conversion == UTF16_INVALID
                ? "batch delete path contains invalid UTF-16"
                : "unable to allocate UTF-8 batch delete path");
        goto cleanup_delete_many;
      }
      converted_count = i + 1;
      path_views[i] = owned_utf8_view(&path_utf8[i]);
    }
  }

  result->status = api.operator_delete_many(
      operator_->operator_, path_views, (uint64_t)(uint32_t)path_count,
      &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK && result->error != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_delete_many:
  for (int32_t i = 0; i < converted_count; ++i) {
    owned_utf8_free(&path_utf8[i]);
  }
  free(path_views);
  free(path_utf8);
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_copier(
    moonbit_opendal_operator_t *operator_, moonbit_string_t source,
    moonbit_string_t destination) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t source_utf8 = {0};
  owned_utf8_t destination_utf8 = {0};
  utf16_result_t conversion;

  result->status = load_copier_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(source, &source_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "source path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 source path");
    goto cleanup_copier;
  }
  conversion = utf16_to_utf8(destination, &destination_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID
            ? "destination path contains invalid UTF-16"
            : "unable to allocate UTF-8 destination path");
    goto cleanup_copier;
  }
  {
    opendal_mbt_bytes_view_v1_t source_view = owned_utf8_view(&source_utf8);
    opendal_mbt_bytes_view_v1_t destination_view =
        owned_utf8_view(&destination_utf8);
    result->status = api.operator_copier(
        operator_->operator_, &source_view, &destination_view, &result->copier,
        &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if (result->copier == NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->copier != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup_copier:
  owned_utf8_free(&destination_utf8);
  owned_utf8_free(&source_utf8);
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_copier_next(moonbit_opendal_copier_t *copier) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  uint64_t bytes = 0;

  result->status = load_copier_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (copier == NULL || copier->copier == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "copier is closed");
    return result;
  }
  result->status =
      api.copier_next(copier->copier, &bytes, &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if (result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    } else {
      result->uint64_value = bytes;
      result->has_uint64 = true;
    }
  } else if (bytes != 0 ||
             (result->status == OPENDAL_MBT_STATUS_END &&
              result->error != NULL)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_copier_finish(moonbit_opendal_copier_t *copier) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;

  result->status = load_copier_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (copier == NULL || copier->copier == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "copier is closed");
    return result;
  }
  result->status = api.copier_finish(copier->copier, &result->metadata,
                                     &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if (result->metadata == NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->metadata != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *
moonbit_opendal_copier_abort(moonbit_opendal_copier_t *copier) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;

  result->status = load_copier_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (copier == NULL || copier->copier == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "copier is closed");
    return result;
  }
  result->status = api.copier_abort(copier->copier, &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK && result->error != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_operator_lister(
    moonbit_opendal_operator_t *operator_, moonbit_string_t path,
    int32_t recursive, int32_t has_limit, uint64_t limit,
    int32_t has_start_after, moonbit_string_t start_after) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  owned_utf8_t path_utf8 = {0};
  owned_utf8_t start_after_utf8 = {0};
  opendal_mbt_list_options_v1_t options;
  utf16_result_t conversion;
  if ((recursive != 0 && recursive != 1) ||
      (has_limit != 0 && has_limit != 1) ||
      (has_start_after != 0 && has_start_after != 1)) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    return result;
  }
  result->status = load_listing_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (operator_ == NULL || operator_->operator_ == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "operator is closed");
    return result;
  }
  conversion = utf16_to_utf8(path, &path_utf8);
  if (conversion != UTF16_OK) {
    result_set_local_error(
        result,
        conversion == UTF16_INVALID ? OPENDAL_MBT_ERROR_INVALID_ARGUMENT
                                    : OPENDAL_MBT_ERROR_UNEXPECTED,
        conversion == UTF16_INVALID ? "InvalidArgument" : "Unexpected",
        conversion == UTF16_INVALID ? "path contains invalid UTF-16"
                                    : "unable to allocate UTF-8 path");
    goto cleanup;
  }
  if (!convert_optional_utf8(
          result, has_start_after != 0, start_after,
          "list start_after contains invalid UTF-16",
          "unable to allocate UTF-8 list start_after", &start_after_utf8)) {
    goto cleanup;
  }
  memset(&options, 0, sizeof(options));
  options.struct_size = (uint32_t)sizeof(options);
  options.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  if (recursive != 0) {
    options.flags = OPENDAL_MBT_LIST_RECURSIVE;
  }
  if (has_limit != 0) {
    options.present_bits |= OPENDAL_MBT_LIST_LIMIT_PRESENT;
    options.limit = limit;
  }
  if (has_start_after != 0) {
    options.present_bits |= OPENDAL_MBT_LIST_START_AFTER_PRESENT;
    options.start_after = owned_utf8_view(&start_after_utf8);
  }
  {
    opendal_mbt_bytes_view_v1_t path_view = owned_utf8_view(&path_utf8);
    result->status = api.operator_lister(operator_->operator_, &path_view,
                                         &options, &result->lister,
                                         &result->error);
  }
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if (result->lister == NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->lister != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }

cleanup:
  owned_utf8_free(&path_utf8);
  owned_utf8_free(&start_after_utf8);
  return result;
}

MOONBIT_FFI_EXPORT moonbit_opendal_result_t *moonbit_opendal_lister_next(
    moonbit_opendal_lister_t *lister) {
  moonbit_opendal_result_t *result = result_new();
  opendal_mbt_api_v1_t api;
  result->status = load_listing_api(&api);
  if (result->status != OPENDAL_MBT_STATUS_OK) {
    return result;
  }
  if (lister == NULL || lister->lister == NULL) {
    result_set_local_error(result, OPENDAL_MBT_ERROR_RESOURCE_CLOSED,
                           "ResourceClosed", "lister is closed");
    return result;
  }
  result->status =
      api.lister_next(lister->lister, &result->entry, &result->error);
  if (result->status == OPENDAL_MBT_STATUS_OK) {
    if (result->entry == NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->status == OPENDAL_MBT_STATUS_END) {
    if (result->entry != NULL || result->error != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
  } else if (result->entry != NULL) {
    result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
  }
  return result;
}

MOONBIT_FFI_EXPORT void moonbit_opendal_lister_close(
    moonbit_opendal_lister_t *lister) {
  opendal_mbt_api_v1_t api;
  if (lister != NULL && lister->lister != NULL &&
      load_listing_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.lister_close(lister->lister);
  }
}

MOONBIT_FFI_EXPORT uint32_t
moonbit_opendal_result_status(moonbit_opendal_result_t *result) {
  return result == NULL ? OPENDAL_MBT_STATUS_ABI_MISMATCH : result->status;
}

MOONBIT_FFI_EXPORT uint32_t
moonbit_opendal_result_error_kind(moonbit_opendal_result_t *result) {
  opendal_mbt_error_view_v1_t view;
  if (result != NULL && result->local_kind != 0) {
    return result->local_kind;
  }
  if (fill_error_view(result, &view)) {
    return view.kind;
  }
  if (result != NULL && result->status == OPENDAL_MBT_STATUS_ABI_MISMATCH) {
    return OPENDAL_MBT_ERROR_ABI_MISMATCH;
  }
  return OPENDAL_MBT_ERROR_UNEXPECTED;
}

MOONBIT_FFI_EXPORT uint32_t
moonbit_opendal_result_error_status(moonbit_opendal_result_t *result) {
  opendal_mbt_error_view_v1_t view;
  if (result != NULL && result->local_error_status != 0) {
    return result->local_error_status;
  }
  if (fill_error_view(result, &view)) {
    return view.status;
  }
  return OPENDAL_MBT_ERROR_STATUS_PERMANENT;
}

MOONBIT_FFI_EXPORT moonbit_bytes_t
moonbit_opendal_result_error_kind_name(moonbit_opendal_result_t *result) {
  opendal_mbt_error_view_v1_t view;
  if (result != NULL && result->local_kind_name != NULL) {
    return copy_c_string(result->local_kind_name);
  }
  if (fill_error_view(result, &view)) {
    return copy_bytes(view.kind_name.data, view.kind_name.len);
  }
  if (result != NULL && result->status == OPENDAL_MBT_STATUS_ABI_MISMATCH) {
    return copy_c_string("AbiMismatch");
  }
  return copy_c_string("Unexpected");
}

MOONBIT_FFI_EXPORT moonbit_bytes_t
moonbit_opendal_result_error_message(moonbit_opendal_result_t *result) {
  opendal_mbt_error_view_v1_t view;
  if (result != NULL && result->local_message != NULL) {
    return copy_c_string(result->local_message);
  }
  if (fill_error_view(result, &view)) {
    return copy_bytes(view.message.data, view.message.len);
  }
  if (result != NULL && result->status == OPENDAL_MBT_STATUS_PANIC) {
    return copy_c_string("native OpenDAL bridge panicked");
  }
  if (result != NULL && result->status == OPENDAL_MBT_STATUS_ABI_MISMATCH) {
    return copy_c_string("native OpenDAL ABI mismatch");
  }
  return copy_c_string("native OpenDAL bridge failed without error detail");
}

MOONBIT_FFI_EXPORT int32_t
moonbit_opendal_result_take_bool(moonbit_opendal_result_t *result) {
  opendal_mbt_bool_t value;
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      !result->has_bool || (result->bool_value != OPENDAL_MBT_FALSE &&
                            result->bool_value != OPENDAL_MBT_TRUE)) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return 0;
  }
  value = result->bool_value;
  result->bool_value = OPENDAL_MBT_FALSE;
  result->has_bool = false;
  return value == OPENDAL_MBT_TRUE ? 1 : 0;
}

MOONBIT_FFI_EXPORT uint64_t
moonbit_opendal_result_take_uint64(moonbit_opendal_result_t *result) {
  uint64_t value;
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      !result->has_uint64) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return 0;
  }
  value = result->uint64_value;
  result->uint64_value = 0;
  result->has_uint64 = false;
  return value;
}

MOONBIT_FFI_EXPORT moonbit_opendal_operator_t *
moonbit_opendal_result_take_operator(moonbit_opendal_result_t *result) {
  moonbit_opendal_operator_t *operator_ = operator_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->operator_ == NULL || result->info == NULL) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return operator_;
  }
  operator_->operator_ = result->operator_;
  operator_->info = result->info;
  result->operator_ = NULL;
  result->info = NULL;
  return operator_;
}

MOONBIT_FFI_EXPORT moonbit_opendal_lister_t *
moonbit_opendal_result_take_lister(moonbit_opendal_result_t *result) {
  moonbit_opendal_lister_t *lister = lister_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->lister == NULL) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return lister;
  }
  lister->lister = result->lister;
  result->lister = NULL;
  return lister;
}

MOONBIT_FFI_EXPORT moonbit_opendal_reader_t *
moonbit_opendal_result_take_reader(moonbit_opendal_result_t *result) {
  moonbit_opendal_reader_t *reader = reader_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->reader == NULL) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return reader;
  }
  reader->reader = result->reader;
  result->reader = NULL;
  return reader;
}

MOONBIT_FFI_EXPORT moonbit_opendal_read_stream_t *
moonbit_opendal_result_take_read_stream(moonbit_opendal_result_t *result) {
  moonbit_opendal_read_stream_t *stream = read_stream_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->read_stream == NULL) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return stream;
  }
  stream->stream = result->read_stream;
  result->read_stream = NULL;
  return stream;
}

MOONBIT_FFI_EXPORT moonbit_opendal_writer_t *
moonbit_opendal_result_take_writer(moonbit_opendal_result_t *result) {
  moonbit_opendal_writer_t *writer = writer_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->writer == NULL) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return writer;
  }
  writer->writer = result->writer;
  result->writer = NULL;
  return writer;
}

MOONBIT_FFI_EXPORT moonbit_opendal_copier_t *
moonbit_opendal_result_take_copier(moonbit_opendal_result_t *result) {
  moonbit_opendal_copier_t *copier = copier_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->copier == NULL) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return copier;
  }
  copier->copier = result->copier;
  result->copier = NULL;
  return copier;
}

MOONBIT_FFI_EXPORT moonbit_opendal_entry_t *
moonbit_opendal_result_take_entry(moonbit_opendal_result_t *result) {
  moonbit_opendal_entry_t *entry = entry_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->entry == NULL || !validate_entry_snapshot(result->entry)) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return entry;
  }
  entry->entry = result->entry;
  result->entry = NULL;
  return entry;
}

MOONBIT_FFI_EXPORT moonbit_bytes_t
moonbit_opendal_result_take_bytes(moonbit_opendal_result_t *result) {
  opendal_mbt_api_v1_t api;
  uint64_t len = 0;
  uint64_t required = 0;
  moonbit_bytes_t bytes;
  opendal_mbt_status_t status;
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->buffer == NULL) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return moonbit_make_bytes(0, 0);
  }
  status = load_api(&api, false);
  if (status != OPENDAL_MBT_STATUS_OK) {
    result->status = status;
    return moonbit_make_bytes(0, 0);
  }
  status = api.buffer_len(result->buffer, &len);
  if (status != OPENDAL_MBT_STATUS_OK || len > (uint64_t)INT32_MAX) {
    result->status = status == OPENDAL_MBT_STATUS_OK
                         ? OPENDAL_MBT_STATUS_ABI_MISMATCH
                         : status;
    return moonbit_make_bytes(0, 0);
  }
  if (len == 0) {
    status = api.buffer_copy(result->buffer, NULL, 0, &required);
    if (status != OPENDAL_MBT_STATUS_OK || required != 0) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
      return moonbit_make_bytes(0, 0);
    }
    bytes = moonbit_make_bytes(0, 0);
  } else {
    bytes = moonbit_make_bytes((int32_t)len, 0);
    status = api.buffer_copy(result->buffer, bytes, len, &required);
    if (status != OPENDAL_MBT_STATUS_OK || required != len) {
      moonbit_decref(bytes);
      result->status = status == OPENDAL_MBT_STATUS_OK
                           ? OPENDAL_MBT_STATUS_ABI_MISMATCH
                           : status;
      return moonbit_make_bytes(0, 0);
    }
  }
  api.buffer_free(result->buffer);
  result->buffer = NULL;
  return bytes;
}

MOONBIT_FFI_EXPORT moonbit_opendal_metadata_t *
moonbit_opendal_result_take_metadata(moonbit_opendal_result_t *result) {
  moonbit_opendal_metadata_t *metadata = metadata_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->metadata == NULL ||
      !validate_metadata_snapshot(result->metadata)) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return metadata;
  }
  metadata->metadata = result->metadata;
  result->metadata = NULL;
  return metadata;
}

MOONBIT_FFI_EXPORT moonbit_opendal_presigned_request_t *
moonbit_opendal_result_take_presigned_request(
    moonbit_opendal_result_t *result) {
  moonbit_opendal_presigned_request_t *request =
      presigned_request_new_external();
  if (result == NULL || result->status != OPENDAL_MBT_STATUS_OK ||
      result->presigned_request == NULL ||
      !validate_presigned_request_snapshot(result->presigned_request)) {
    if (result != NULL) {
      result->status = OPENDAL_MBT_STATUS_ABI_MISMATCH;
    }
    return request;
  }
  request->request = result->presigned_request;
  result->presigned_request = NULL;
  return request;
}

MOONBIT_FFI_EXPORT void
moonbit_opendal_result_release(moonbit_opendal_result_t *result) {
  if (result != NULL) {
    release_result_payload(result);
  }
}

MOONBIT_FFI_EXPORT moonbit_bytes_t moonbit_opendal_presigned_request_method(
    moonbit_opendal_presigned_request_t *request) {
  opendal_mbt_presigned_request_view_v1_t view;
  return fill_presigned_request_view(request, &view)
             ? copy_bytes(view.method.data, view.method.len)
             : moonbit_make_bytes(0, 0);
}

MOONBIT_FFI_EXPORT moonbit_bytes_t moonbit_opendal_presigned_request_uri(
    moonbit_opendal_presigned_request_t *request) {
  opendal_mbt_presigned_request_view_v1_t view;
  return fill_presigned_request_view(request, &view)
             ? copy_bytes(view.uri.data, view.uri.len)
             : moonbit_make_bytes(0, 0);
}

MOONBIT_FFI_EXPORT uint64_t
moonbit_opendal_presigned_request_header_count(
    moonbit_opendal_presigned_request_t *request) {
  opendal_mbt_presigned_request_view_v1_t view;
  return fill_presigned_request_view(request, &view) ? view.header_count : 0;
}

MOONBIT_FFI_EXPORT moonbit_bytes_t
moonbit_opendal_presigned_request_header_name(
    moonbit_opendal_presigned_request_t *request, int32_t index) {
  opendal_mbt_presigned_header_view_v1_t view;
  if (index < 0 ||
      !fill_presigned_header_view(request, (uint64_t)(uint32_t)index, &view)) {
    return moonbit_make_bytes(0, 0);
  }
  return copy_bytes(view.name.data, view.name.len);
}

MOONBIT_FFI_EXPORT moonbit_bytes_t
moonbit_opendal_presigned_request_header_value(
    moonbit_opendal_presigned_request_t *request, int32_t index) {
  opendal_mbt_presigned_header_view_v1_t view;
  if (index < 0 ||
      !fill_presigned_header_view(request, (uint64_t)(uint32_t)index, &view)) {
    return moonbit_make_bytes(0, 0);
  }
  return copy_bytes(view.value.data, view.value.len);
}

MOONBIT_FFI_EXPORT void moonbit_opendal_presigned_request_release(
    moonbit_opendal_presigned_request_t *request) {
  opendal_mbt_api_v1_t api;
  if (request != NULL && request->request != NULL &&
      load_presign_api(&api) == OPENDAL_MBT_STATUS_OK) {
    api.presigned_request_free(request->request);
    request->request = NULL;
  }
}

MOONBIT_FFI_EXPORT moonbit_bytes_t
moonbit_opendal_entry_path(moonbit_opendal_entry_t *entry) {
  opendal_mbt_entry_view_v1_t view;
  return fill_entry_view(entry, &view)
             ? copy_bytes(view.path.data, view.path.len)
             : moonbit_make_bytes(0, 0);
}

MOONBIT_FFI_EXPORT moonbit_bytes_t
moonbit_opendal_entry_name(moonbit_opendal_entry_t *entry) {
  opendal_mbt_entry_view_v1_t view;
  return fill_entry_view(entry, &view)
             ? copy_bytes(view.name.data, view.name.len)
             : moonbit_make_bytes(0, 0);
}

MOONBIT_FFI_EXPORT moonbit_opendal_metadata_t *
moonbit_opendal_entry_take_metadata(moonbit_opendal_entry_t *entry) {
  moonbit_opendal_metadata_t *metadata = metadata_new_external();
  if (entry != NULL && entry->entry != NULL) {
    metadata->entry = entry->entry;
    entry->entry = NULL;
  }
  return metadata;
}

MOONBIT_FFI_EXPORT void
moonbit_opendal_entry_release(moonbit_opendal_entry_t *entry) {
  if (entry != NULL) {
    entry_finalize(entry);
  }
}

MOONBIT_FFI_EXPORT moonbit_bytes_t moonbit_opendal_operator_info_scheme(
    moonbit_opendal_operator_t *operator_) {
  opendal_mbt_operator_info_view_v1_t view;
  return fill_operator_info_view(operator_, &view)
             ? copy_bytes(view.scheme.data, view.scheme.len)
             : moonbit_make_bytes(0, 0);
}

MOONBIT_FFI_EXPORT moonbit_bytes_t moonbit_opendal_operator_info_root(
    moonbit_opendal_operator_t *operator_) {
  opendal_mbt_operator_info_view_v1_t view;
  return fill_operator_info_view(operator_, &view)
             ? copy_bytes(view.root.data, view.root.len)
             : moonbit_make_bytes(0, 0);
}

MOONBIT_FFI_EXPORT moonbit_bytes_t moonbit_opendal_operator_info_name(
    moonbit_opendal_operator_t *operator_) {
  opendal_mbt_operator_info_view_v1_t view;
  return fill_operator_info_view(operator_, &view)
             ? copy_bytes(view.name.data, view.name.len)
             : moonbit_make_bytes(0, 0);
}

MOONBIT_FFI_EXPORT moonbit_opendal_capability_t *
moonbit_opendal_operator_info_capability(
    moonbit_opendal_operator_t *operator_) {
  moonbit_opendal_capability_t *capability = capability_new_external();
  opendal_mbt_operator_info_view_v1_t view;
  if (fill_operator_info_view(operator_, &view)) {
    memcpy(capability->words, view.capability.words, sizeof(capability->words));
  }
  return capability;
}

MOONBIT_FFI_EXPORT uint64_t moonbit_opendal_capability_word(
    moonbit_opendal_capability_t *capability, int32_t index) {
  if (capability == NULL || index < 0 || index >= 4) {
    return 0;
  }
  return capability->words[index];
}

MOONBIT_FFI_EXPORT uint64_t moonbit_opendal_metadata_present_bits(
    moonbit_opendal_metadata_t *metadata) {
  opendal_mbt_metadata_view_v1_t view;
  return fill_metadata_view(metadata, &view) ? view.present_bits : 0;
}

MOONBIT_FFI_EXPORT uint32_t moonbit_opendal_metadata_mode(
    moonbit_opendal_metadata_t *metadata) {
  opendal_mbt_metadata_view_v1_t view;
  return fill_metadata_view(metadata, &view) ? view.mode
                                             : OPENDAL_MBT_ENTRY_MODE_UNKNOWN;
}

MOONBIT_FFI_EXPORT uint64_t moonbit_opendal_metadata_content_length(
    moonbit_opendal_metadata_t *metadata) {
  opendal_mbt_metadata_view_v1_t view;
  return fill_metadata_view(metadata, &view) ? view.content_length : 0;
}

MOONBIT_FFI_EXPORT int32_t moonbit_opendal_metadata_is_current(
    moonbit_opendal_metadata_t *metadata) {
  opendal_mbt_metadata_view_v1_t view;
  return fill_metadata_view(metadata, &view) &&
                 view.is_current == OPENDAL_MBT_TRUE
             ? 1
             : 0;
}

MOONBIT_FFI_EXPORT int32_t moonbit_opendal_metadata_is_deleted(
    moonbit_opendal_metadata_t *metadata) {
  opendal_mbt_metadata_view_v1_t view;
  return fill_metadata_view(metadata, &view) &&
                 view.is_deleted == OPENDAL_MBT_TRUE
             ? 1
             : 0;
}

MOONBIT_FFI_EXPORT int64_t moonbit_opendal_metadata_last_modified_seconds(
    moonbit_opendal_metadata_t *metadata) {
  opendal_mbt_metadata_view_v1_t view;
  return fill_metadata_view(metadata, &view)
             ? view.last_modified.unix_seconds
             : 0;
}

MOONBIT_FFI_EXPORT uint32_t
moonbit_opendal_metadata_last_modified_nanoseconds(
    moonbit_opendal_metadata_t *metadata) {
  opendal_mbt_metadata_view_v1_t view;
  return fill_metadata_view(metadata, &view)
             ? view.last_modified.nanoseconds
             : 0;
}

#define DEFINE_METADATA_TEXT_GETTER(name, field)                              \
  MOONBIT_FFI_EXPORT moonbit_bytes_t moonbit_opendal_metadata_##name(         \
      moonbit_opendal_metadata_t *metadata) {                                 \
    opendal_mbt_metadata_view_v1_t view;                                      \
    return fill_metadata_view(metadata, &view)                                \
               ? copy_bytes(view.field.data, view.field.len)                  \
               : moonbit_make_bytes(0, 0);                                    \
  }

DEFINE_METADATA_TEXT_GETTER(cache_control, cache_control)
DEFINE_METADATA_TEXT_GETTER(content_disposition, content_disposition)
DEFINE_METADATA_TEXT_GETTER(content_encoding, content_encoding)
DEFINE_METADATA_TEXT_GETTER(content_md5, content_md5)
DEFINE_METADATA_TEXT_GETTER(content_type, content_type)
DEFINE_METADATA_TEXT_GETTER(etag, etag)
DEFINE_METADATA_TEXT_GETTER(version, version)

MOONBIT_FFI_EXPORT void
moonbit_opendal_metadata_release(moonbit_opendal_metadata_t *metadata) {
  metadata_finalize(metadata);
}
