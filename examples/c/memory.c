/*
 * End-to-end C consumer for the OpenDAL MoonBit binding ABI v1.
 *
 * This example deliberately uses binary data with embedded NUL bytes. It also
 * performs one expected failing read so the owned error/view/free path is
 * exercised, rather than merely present as dead error-handling code.
 */

#include "opendal_mbt.h"

#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define COPY_TAIL_CANARY_BYTES ((size_t)8)
#define COPY_TAIL_CANARY_VALUE UINT8_C(0xa5)

static opendal_mbt_bytes_view_v1_t bytes_view(const void *data, uint64_t len) {
  opendal_mbt_bytes_view_v1_t view;
  view.data = (const uint8_t *)data;
  view.len = len;
  return view;
}

static const char *status_name(opendal_mbt_status_t status) {
  switch (status) {
  case OPENDAL_MBT_STATUS_OK:
    return "OK";
  case OPENDAL_MBT_STATUS_END:
    return "END";
  case OPENDAL_MBT_STATUS_ERROR:
    return "ERROR";
  case OPENDAL_MBT_STATUS_BUFFER_TOO_SMALL:
    return "BUFFER_TOO_SMALL";
  case OPENDAL_MBT_STATUS_ABI_MISMATCH:
    return "ABI_MISMATCH";
  case OPENDAL_MBT_STATUS_PANIC:
    return "PANIC";
  default:
    return "UNKNOWN_STATUS";
  }
}

static const char *entry_mode_name(opendal_mbt_entry_mode_t mode) {
  switch (mode) {
  case OPENDAL_MBT_ENTRY_MODE_UNKNOWN:
    return "unknown";
  case OPENDAL_MBT_ENTRY_MODE_FILE:
    return "file";
  case OPENDAL_MBT_ENTRY_MODE_DIRECTORY:
    return "directory";
  default:
    return "future-mode";
  }
}

static int view_is_valid(opendal_mbt_bytes_view_v1_t view) {
  return view.len == 0 || (view.data != NULL && view.len <= (uint64_t)SIZE_MAX);
}

static void print_view(FILE *stream, opendal_mbt_bytes_view_v1_t view) {
  if (view.len != 0) {
    (void)fwrite(view.data, 1, (size_t)view.len, stream);
  }
}

static int view_equals(opendal_mbt_bytes_view_v1_t view, const uint8_t *data,
                       size_t len) {
  if (!view_is_valid(view) || view.len != (uint64_t)len) {
    return 0;
  }
  return len == 0 || memcmp(view.data, data, len) == 0;
}

static int api_covers(const opendal_mbt_api_v1_t *api, uint32_t field_end) {
  return api->struct_size >= field_end &&
         api->library_struct_size >= field_end;
}

#define API_HAS(api, field)                                                  \
  (api_covers((api), OPENDAL_MBT_API_V1_FIELD_END(field)) &&                 \
   (api)->field != NULL)

static void report_status(const char *operation, opendal_mbt_status_t status) {
  (void)fprintf(stderr, "%s: %s (%" PRIu32 ")\n", operation,
                status_name(status), status);
}

/* Consumes and frees *slot on every path. */
static int print_and_free_error(const opendal_mbt_api_v1_t *api,
                                const char *operation,
                                opendal_mbt_error_v1_t **slot) {
  opendal_mbt_error_v1_t *error = *slot;
  opendal_mbt_error_view_v1_t view;
  opendal_mbt_status_t status;
  int valid;

  *slot = NULL;
  if (error == NULL) {
    (void)fprintf(stderr, "%s: ERROR returned without an error snapshot\n",
                  operation);
    return 0;
  }

  memset(&view, 0, sizeof(view));
  view.struct_size = (uint32_t)sizeof(view);
  view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  status = api->error_view(error, &view);
  if (status != OPENDAL_MBT_STATUS_OK) {
    (void)fprintf(stderr, "%s: error_view failed: %s (%" PRIu32 ")\n",
                  operation, status_name(status), status);
    api->error_free(error);
    return 0;
  }

  valid = view_is_valid(view.kind_name) && view_is_valid(view.message);
  (void)fprintf(stderr, "%s: OpenDAL error kind=%" PRIu32
                        " status=%" PRIu32 " name=\"",
                operation, view.kind, view.status);
  if (view_is_valid(view.kind_name)) {
    print_view(stderr, view.kind_name);
  } else {
    (void)fputs("<invalid-view>", stderr);
  }
  (void)fputs("\" message=\"", stderr);
  if (view_is_valid(view.message)) {
    print_view(stderr, view.message);
  } else {
    (void)fputs("<invalid-view>", stderr);
  }
  (void)fputs("\"\n", stderr);

  api->error_free(error);
  return valid;
}

/* Accepts only OK with a NULL error output, consuming any unexpected error. */
static int expect_ok(const opendal_mbt_api_v1_t *api, const char *operation,
                     opendal_mbt_status_t status,
                     opendal_mbt_error_v1_t **error) {
  if (status == OPENDAL_MBT_STATUS_OK && *error == NULL) {
    return 1;
  }

  if (status == OPENDAL_MBT_STATUS_OK) {
    (void)fprintf(stderr,
                  "%s: OK returned with an unexpected error snapshot\n",
                  operation);
    (void)print_and_free_error(api, operation, error);
    return 0;
  }

  report_status(operation, status);
  if (*error != NULL) {
    (void)print_and_free_error(api, operation, error);
  } else if (status == OPENDAL_MBT_STATUS_ERROR) {
    (void)fprintf(stderr, "%s: ERROR did not provide the requested snapshot\n",
                  operation);
  }
  return 0;
}

/* Accepts exactly ERROR with one owned error, consuming it after printing. */
static int expect_error(const opendal_mbt_api_v1_t *api, const char *operation,
                        opendal_mbt_status_t status,
                        opendal_mbt_error_v1_t **error) {
  if (status != OPENDAL_MBT_STATUS_ERROR) {
    (void)fprintf(stderr, "%s: expected ERROR, got %s (%" PRIu32 ")\n",
                  operation, status_name(status), status);
    if (*error != NULL) {
      (void)print_and_free_error(api, operation, error);
    }
    return 0;
  }
  return print_and_free_error(api, operation, error);
}

static int show_library_info(const opendal_mbt_api_v1_t *api) {
  opendal_mbt_library_info_view_v1_t view;
  opendal_mbt_status_t status;

  memset(&view, 0, sizeof(view));
  view.struct_size = (uint32_t)sizeof(view);
  view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  status = api->library_info(&view);
  if (status != OPENDAL_MBT_STATUS_OK) {
    report_status("library_info", status);
    return 0;
  }
  if (!view_is_valid(view.binding_version) ||
      !view_is_valid(view.opendal_version) ||
      !view_is_valid(view.service_profile)) {
    (void)fputs("library_info: invalid borrowed byte view\n", stderr);
    return 0;
  }

  (void)fputs("binding ", stdout);
  print_view(stdout, view.binding_version);
  (void)fputs(" (OpenDAL ", stdout);
  print_view(stdout, view.opendal_version);
  (void)fputs(", profile ", stdout);
  print_view(stdout, view.service_profile);
  (void)fputs(")\n", stdout);
  return 1;
}

static int show_operator_info(const opendal_mbt_api_v1_t *api,
                              const opendal_mbt_operator_info_v1_t *info) {
  static const uint8_t expected_scheme[] = {'m', 'e', 'm', 'o', 'r', 'y'};
  const uint64_t required_caps = OPENDAL_MBT_CAP_READ | OPENDAL_MBT_CAP_WRITE;
  opendal_mbt_operator_info_view_v1_t view;
  opendal_mbt_status_t status;

  memset(&view, 0, sizeof(view));
  view.struct_size = (uint32_t)sizeof(view);
  view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  status = api->operator_info_view(info, &view);
  if (status != OPENDAL_MBT_STATUS_OK) {
    report_status("operator_info_view", status);
    return 0;
  }
  if (!view_is_valid(view.scheme) || !view_is_valid(view.root) ||
      !view_is_valid(view.name)) {
    (void)fputs("operator_info_view: invalid borrowed byte view\n", stderr);
    return 0;
  }
  if (!view_equals(view.scheme, expected_scheme, sizeof(expected_scheme))) {
    (void)fputs("operator_info_view: constructor did not return memory scheme\n",
                stderr);
    return 0;
  }
  if ((view.capability.words[0] & required_caps) != required_caps) {
    (void)fputs("operator_info_view: memory lacks read/write capability\n",
                stderr);
    return 0;
  }

  (void)fputs("operator scheme=", stdout);
  print_view(stdout, view.scheme);
  (void)fputs(" root=\"", stdout);
  print_view(stdout, view.root);
  (void)fputs("\" name=\"", stdout);
  print_view(stdout, view.name);
  (void)fprintf(stdout, "\" capability[0]=0x%016" PRIx64 "\n",
                view.capability.words[0]);
  return 1;
}

static int show_metadata(const opendal_mbt_api_v1_t *api,
                         const opendal_mbt_metadata_v1_t *metadata) {
  opendal_mbt_metadata_view_v1_t view;
  opendal_mbt_status_t status;

  memset(&view, 0, sizeof(view));
  view.struct_size = (uint32_t)sizeof(view);
  view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  status = api->metadata_view(metadata, &view);
  if (status != OPENDAL_MBT_STATUS_OK) {
    report_status("metadata_view", status);
    return 0;
  }
  if (!view_is_valid(view.cache_control) ||
      !view_is_valid(view.content_disposition) ||
      !view_is_valid(view.content_encoding) ||
      !view_is_valid(view.content_md5) || !view_is_valid(view.content_type) ||
      !view_is_valid(view.etag) || !view_is_valid(view.version)) {
    (void)fputs("metadata_view: invalid borrowed byte view\n", stderr);
    return 0;
  }

  (void)fprintf(stdout,
                "write metadata mode=%s content_length=%" PRIu64
                " present_bits=0x%016" PRIx64 "\n",
                entry_mode_name(view.mode), view.content_length,
                view.present_bits);
  return 1;
}

/*
 * Copies a Rust-owned buffer via the sizing-query protocol. On success,
 * *out_data is C-owned malloc memory and *out_len is its meaningful prefix.
 */
static int copy_buffer(const opendal_mbt_api_v1_t *api,
                       const opendal_mbt_buffer_v1_t *buffer,
                       uint8_t **out_data, uint64_t *out_len) {
  opendal_mbt_status_t status;
  opendal_mbt_status_t expected_query_status;
  uint64_t reported_len = 0;
  uint64_t required = 0;
  uint64_t copied = 0;
  size_t capacity;
  uint8_t *destination;
  size_t index;

  *out_data = NULL;
  *out_len = 0;

  status = api->buffer_len(buffer, &reported_len);
  if (status != OPENDAL_MBT_STATUS_OK) {
    report_status("buffer_len", status);
    return 0;
  }

  status = api->buffer_copy(buffer, NULL, 0, &required);
  expected_query_status = reported_len == 0
                              ? OPENDAL_MBT_STATUS_OK
                              : OPENDAL_MBT_STATUS_BUFFER_TOO_SMALL;
  if (status != expected_query_status) {
    (void)fprintf(stderr,
                  "buffer_copy(size query): expected %s, got %s (%" PRIu32
                  ")\n",
                  status_name(expected_query_status), status_name(status),
                  status);
    return 0;
  }
  if (required != reported_len) {
    (void)fprintf(stderr,
                  "buffer_copy(size query): buffer_len=%" PRIu64
                  " but required=%" PRIu64 "\n",
                  reported_len, required);
    return 0;
  }
  if (required > (uint64_t)(SIZE_MAX - COPY_TAIL_CANARY_BYTES) ||
      required > UINT64_MAX - (uint64_t)COPY_TAIL_CANARY_BYTES) {
    (void)fputs("buffer_copy: result cannot fit in C address space\n", stderr);
    return 0;
  }

  capacity = (size_t)required + COPY_TAIL_CANARY_BYTES;
  destination = (uint8_t *)malloc(capacity);
  if (destination == NULL) {
    (void)fprintf(stderr, "malloc(%zu) failed\n", capacity);
    return 0;
  }
  memset(destination, COPY_TAIL_CANARY_VALUE, capacity);

  status = api->buffer_copy(buffer, destination, (uint64_t)capacity, &copied);
  if (status != OPENDAL_MBT_STATUS_OK) {
    report_status("buffer_copy(copy)", status);
    free(destination);
    return 0;
  }
  if (copied != required) {
    (void)fprintf(stderr,
                  "buffer_copy(copy): required=%" PRIu64
                  " but copied=%" PRIu64 "\n",
                  required, copied);
    free(destination);
    return 0;
  }
  for (index = (size_t)required; index < capacity; ++index) {
    if (destination[index] != COPY_TAIL_CANARY_VALUE) {
      (void)fputs("buffer_copy(copy): wrote past the required prefix\n",
                  stderr);
      free(destination);
      return 0;
    }
  }

  *out_data = destination;
  *out_len = copied;
  return 1;
}

int main(void) {
  static const uint8_t memory_scheme[] = {'m', 'e', 'm', 'o', 'r', 'y'};
  static const uint8_t object_path[] = "examples/roundtrip.bin";
  static const uint8_t aborted_path[] = "examples/aborted.bin";
  static const uint8_t missing_path[] = "examples/missing.bin";
  static const uint8_t payload[] = {
      UINT8_C(0x4f), UINT8_C(0x70), UINT8_C(0x65), UINT8_C(0x6e),
      UINT8_C(0x44), UINT8_C(0x41), UINT8_C(0x4c), UINT8_C(0x00),
      UINT8_C(0xff), UINT8_C(0x00), UINT8_C(0x7f), UINT8_C(0x80),
  };
  const opendal_mbt_bytes_view_v1_t scheme =
      bytes_view(memory_scheme, (uint64_t)sizeof(memory_scheme));
  const opendal_mbt_bytes_view_v1_t path =
      bytes_view(object_path, (uint64_t)(sizeof(object_path) - 1));
  const opendal_mbt_bytes_view_v1_t absent_path =
      bytes_view(missing_path, (uint64_t)(sizeof(missing_path) - 1));
  const opendal_mbt_bytes_view_v1_t discarded_path =
      bytes_view(aborted_path, (uint64_t)(sizeof(aborted_path) - 1));
  const opendal_mbt_bytes_view_v1_t data =
      bytes_view(payload, (uint64_t)sizeof(payload));
  const uint64_t required_features = OPENDAL_MBT_FEATURE_BASE |
                                     OPENDAL_MBT_FEATURE_WHOLE_OBJECT |
                                     OPENDAL_MBT_FEATURE_READ_STREAM |
                                     OPENDAL_MBT_FEATURE_CHUNKED_WRITER |
                                     OPENDAL_MBT_FEATURE_WRITER_ABORT |
                                     OPENDAL_MBT_FEATURE_LAYERS;
  opendal_mbt_api_v1_t api;
  opendal_mbt_operator_v1_t *operator_ = NULL;
  opendal_mbt_operator_v1_t *base_operator = NULL;
  opendal_mbt_operator_v1_t *timeout_operator = NULL;
  opendal_mbt_operator_info_v1_t *operator_info = NULL;
  opendal_mbt_metadata_v1_t *metadata = NULL;
  opendal_mbt_buffer_v1_t *buffer = NULL;
  opendal_mbt_read_stream_v1_t *read_stream = NULL;
  opendal_mbt_writer_v1_t *writer = NULL;
  opendal_mbt_error_v1_t *error = NULL;
  opendal_mbt_read_stream_options_v1_t stream_options;
  uint8_t *roundtrip = NULL;
  uint64_t roundtrip_len = 0;
  uint8_t stream_roundtrip[sizeof(payload)];
  size_t stream_roundtrip_len = 0;
  opendal_mbt_status_t status;
  int api_ready = 0;
  int result = EXIT_FAILURE;

  memset(&api, 0, sizeof(api));
  api.struct_size = OPENDAL_MBT_API_V1_SIZE;
  api.requested_major = OPENDAL_MBT_ABI_V1_MAJOR;
  status = opendal_mbt_get_api(&api);
  if (status != OPENDAL_MBT_STATUS_OK) {
    report_status("opendal_mbt_get_api", status);
    goto cleanup;
  }
  if (api.struct_size != OPENDAL_MBT_API_V1_SIZE ||
      api.library_struct_size < OPENDAL_MBT_API_V1_PREFIX_SIZE) {
    (void)fputs("opendal_mbt_get_api: invalid negotiated table prefix\n",
                stderr);
    goto cleanup;
  }
  if ((api.feature_bits & required_features) != required_features) {
    (void)fprintf(stderr,
                  "opendal_mbt_get_api: required operation/lifecycle/layer "
                  "groups unavailable, "
                  "got 0x%016"
                  PRIx64 "\n",
                  api.feature_bits);
    goto cleanup;
  }

#define REQUIRE_API_FIELD(field)                                             \
  do {                                                                       \
    if (!API_HAS(&api, field)) {                                             \
      (void)fprintf(stderr,                                                   \
                    "opendal_mbt_get_api: unavailable field '%s' (end=%"     \
                    PRIu32 ", library_size=%" PRIu32 ")\n",                \
                    #field, OPENDAL_MBT_API_V1_FIELD_END(field),              \
                    api.library_struct_size);                                \
      goto cleanup;                                                          \
    }                                                                        \
  } while (0)

  REQUIRE_API_FIELD(library_info);
  REQUIRE_API_FIELD(error_view);
  REQUIRE_API_FIELD(error_free);
  REQUIRE_API_FIELD(buffer_len);
  REQUIRE_API_FIELD(buffer_copy);
  REQUIRE_API_FIELD(buffer_free);
  REQUIRE_API_FIELD(metadata_view);
  REQUIRE_API_FIELD(metadata_free);
  REQUIRE_API_FIELD(operator_info_view);
  REQUIRE_API_FIELD(operator_info_free);
  REQUIRE_API_FIELD(operator_new);
  REQUIRE_API_FIELD(operator_free);
  REQUIRE_API_FIELD(operator_read);
  REQUIRE_API_FIELD(operator_write);
  REQUIRE_API_FIELD(operator_read_stream);
  REQUIRE_API_FIELD(read_stream_next);
  REQUIRE_API_FIELD(read_stream_close);
  REQUIRE_API_FIELD(read_stream_free);
  REQUIRE_API_FIELD(operator_writer);
  REQUIRE_API_FIELD(writer_write);
  REQUIRE_API_FIELD(writer_close);
  REQUIRE_API_FIELD(writer_free);
  REQUIRE_API_FIELD(writer_abort);
  REQUIRE_API_FIELD(operator_with_timeout);
  REQUIRE_API_FIELD(operator_with_retry);
#undef REQUIRE_API_FIELD

  api_ready = 1;
  if (!show_library_info(&api)) {
    goto cleanup;
  }
  if (api.max_output_bytes < (uint64_t)sizeof(payload)) {
    (void)fprintf(stderr,
                  "library max_output_bytes=%" PRIu64
                  " is smaller than example payload\n",
                  api.max_output_bytes);
    goto cleanup;
  }

  status = api.operator_new(&scheme, NULL, 0, &base_operator, &operator_info,
                            &error);
  if (!expect_ok(&api, "operator_new(memory)", status, &error)) {
    goto cleanup;
  }
  if (base_operator == NULL || operator_info == NULL) {
    (void)fputs("operator_new(memory): OK returned incomplete outputs\n",
                stderr);
    goto cleanup;
  }
  if (!show_operator_info(&api, operator_info)) {
    goto cleanup;
  }
  api.operator_info_free(operator_info);
  operator_info = NULL;

  status = api.operator_with_timeout(base_operator, UINT64_C(5000),
                                     UINT64_C(2000), &timeout_operator,
                                     &operator_info,
                                     &error);
  if (!expect_ok(&api, "operator_with_timeout", status, &error) ||
      timeout_operator == NULL || operator_info == NULL) {
    goto cleanup;
  }
  api.operator_info_free(operator_info);
  operator_info = NULL;
  status = api.operator_with_retry(timeout_operator, UINT32_C(3), UINT64_C(1),
                                   UINT64_C(5), OPENDAL_MBT_FALSE, &operator_,
                                   &operator_info,
                                   &error);
  if (!expect_ok(&api, "operator_with_retry", status, &error) ||
      operator_ == NULL || operator_info == NULL) {
    goto cleanup;
  }
  api.operator_info_free(operator_info);
  operator_info = NULL;

  /* The composed handle owns its stack independently of both borrowed inputs. */
  api.operator_free(base_operator);
  base_operator = NULL;
  api.operator_free(timeout_operator);
  timeout_operator = NULL;

  /* Exercise the error snapshot, borrowed error view, and paired free. */
  status = api.operator_read(operator_, &absent_path, NULL,
                             (uint64_t)sizeof(payload), &buffer, &error);
  if (buffer != NULL) {
    (void)fputs("operator_read(missing): ERROR path returned a buffer\n",
                stderr);
    api.buffer_free(buffer);
    buffer = NULL;
    if (error != NULL) {
      (void)print_and_free_error(&api, "operator_read(missing)", &error);
    }
    goto cleanup;
  }
  if (!expect_error(&api, "operator_read(missing, expected)", status,
                    &error)) {
    goto cleanup;
  }

  status = api.operator_write(operator_, &path, &data, NULL, &metadata,
                              &error);
  if (!expect_ok(&api, "operator_write", status, &error)) {
    goto cleanup;
  }
  if (metadata == NULL) {
    (void)fputs("operator_write: OK returned without metadata\n", stderr);
    goto cleanup;
  }
  if (!show_metadata(&api, metadata)) {
    goto cleanup;
  }
  api.metadata_free(metadata);
  metadata = NULL;

  status = api.operator_read(operator_, &path, NULL,
                             (uint64_t)sizeof(payload), &buffer, &error);
  if (!expect_ok(&api, "operator_read(roundtrip)", status, &error)) {
    goto cleanup;
  }
  if (buffer == NULL) {
    (void)fputs("operator_read(roundtrip): OK returned without a buffer\n",
                stderr);
    goto cleanup;
  }
  if (!copy_buffer(&api, buffer, &roundtrip, &roundtrip_len)) {
    goto cleanup;
  }
  api.buffer_free(buffer);
  buffer = NULL;

  if (roundtrip_len != (uint64_t)sizeof(payload) ||
      memcmp(roundtrip, payload, sizeof(payload)) != 0) {
    (void)fputs("binary roundtrip mismatch\n", stderr);
    goto cleanup;
  }

  memset(&stream_options, 0, sizeof(stream_options));
  stream_options.struct_size = (uint32_t)sizeof(stream_options);
  stream_options.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  stream_options.range.struct_size =
      (uint32_t)sizeof(stream_options.range);
  stream_options.range.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  stream_options.range.kind = OPENDAL_MBT_RANGE_FULL;
  stream_options.chunk_size = UINT64_C(5);
  status = api.operator_read_stream(operator_, &path, &stream_options,
                                    &read_stream, &error);
  if (!expect_ok(&api, "operator_read_stream", status, &error) ||
      read_stream == NULL) {
    goto cleanup;
  }
  for (;;) {
    uint8_t *chunk = NULL;
    uint64_t chunk_len = 0;

    status = api.read_stream_next(read_stream, api.max_output_bytes, &buffer,
                                  &error);
    if (status == OPENDAL_MBT_STATUS_END) {
      if (buffer != NULL || error != NULL) {
        (void)fputs("read_stream_next: END returned outputs\n", stderr);
        free(chunk);
        goto cleanup;
      }
      break;
    }
    if (!expect_ok(&api, "read_stream_next", status, &error) ||
        buffer == NULL) {
      free(chunk);
      goto cleanup;
    }
    if (!copy_buffer(&api, buffer, &chunk, &chunk_len)) {
      free(chunk);
      goto cleanup;
    }
    api.buffer_free(buffer);
    buffer = NULL;
    if (chunk_len == 0 || chunk_len > stream_options.chunk_size ||
        chunk_len > (uint64_t)(sizeof(stream_roundtrip) -
                               stream_roundtrip_len)) {
      (void)fputs("read_stream_next: invalid bounded chunk length\n", stderr);
      free(chunk);
      goto cleanup;
    }
    memcpy(stream_roundtrip + stream_roundtrip_len, chunk, (size_t)chunk_len);
    stream_roundtrip_len += (size_t)chunk_len;
    free(chunk);
  }
  if (stream_roundtrip_len != sizeof(payload) ||
      memcmp(stream_roundtrip, payload, sizeof(payload)) != 0) {
    (void)fputs("bounded read stream roundtrip mismatch\n", stderr);
    goto cleanup;
  }
  /* END is stable until close; close itself is idempotent. */
  status = api.read_stream_next(read_stream, api.max_output_bytes, &buffer,
                                &error);
  if (status != OPENDAL_MBT_STATUS_END || buffer != NULL || error != NULL) {
    (void)fputs("read_stream_next: repeated END was not stable\n", stderr);
    goto cleanup;
  }
  api.read_stream_close(read_stream);
  api.read_stream_close(read_stream);
  api.read_stream_free(read_stream);
  read_stream = NULL;

  status = api.operator_writer(operator_, &discarded_path, NULL, &writer,
                               &error);
  if (!expect_ok(&api, "operator_writer(abort)", status, &error) ||
      writer == NULL) {
    goto cleanup;
  }
  status = api.writer_write(writer, &data, &error);
  if (!expect_ok(&api, "writer_write(abort)", status, &error)) {
    goto cleanup;
  }
  status = api.writer_abort(writer, &error);
  if (!expect_ok(&api, "writer_abort", status, &error)) {
    goto cleanup;
  }
  status = api.writer_abort(writer, &error);
  if (!expect_ok(&api, "writer_abort(repeated)", status, &error)) {
    goto cleanup;
  }
  api.writer_free(writer);
  writer = NULL;

  status = api.operator_read(operator_, &discarded_path, NULL,
                             (uint64_t)sizeof(payload), &buffer, &error);
  if (buffer != NULL) {
    (void)fputs("operator_read(aborted): returned a buffer\n", stderr);
    api.buffer_free(buffer);
    buffer = NULL;
    goto cleanup;
  }
  if (!expect_error(&api, "operator_read(aborted, expected)", status,
                    &error)) {
    goto cleanup;
  }

  (void)fprintf(stdout,
                "binary roundtrip OK: %" PRIu64
                " bytes, including embedded NUL and non-UTF-8 bytes\n",
                roundtrip_len);
  result = EXIT_SUCCESS;

cleanup:
  free(roundtrip);
  if (api_ready) {
    if (error != NULL) {
      (void)print_and_free_error(&api, "cleanup", &error);
    }
    if (buffer != NULL) {
      api.buffer_free(buffer);
    }
    if (metadata != NULL) {
      api.metadata_free(metadata);
    }
    if (read_stream != NULL) {
      api.read_stream_free(read_stream);
    }
    if (writer != NULL) {
      api.writer_free(writer);
    }
    if (operator_info != NULL) {
      api.operator_info_free(operator_info);
    }
    if (operator_ != NULL) {
      api.operator_free(operator_);
    }
    if (timeout_operator != NULL) {
      api.operator_free(timeout_operator);
    }
    if (base_operator != NULL) {
      api.operator_free(base_operator);
    }
  }
  return result;
}
