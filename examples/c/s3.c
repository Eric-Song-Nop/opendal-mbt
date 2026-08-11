/* Direct ABI consumer for typed, unsigned S3 construction (no network I/O). */

#include "opendal_mbt.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static opendal_mbt_bytes_view_v1_t text_view(const char *value) {
  opendal_mbt_bytes_view_v1_t view;
  view.data = (const uint8_t *)value;
  view.len = (uint64_t)strlen(value);
  return view;
}

int main(void) {
  opendal_mbt_api_v1_t api;
  opendal_mbt_s3_options_v1_t options;
  opendal_mbt_operator_v1_t *operator_ = NULL;
  opendal_mbt_operator_info_v1_t *info = NULL;
  opendal_mbt_error_v1_t *error = NULL;
  opendal_mbt_operator_info_view_v1_t info_view;
  opendal_mbt_status_t status;
  int result = EXIT_FAILURE;

  memset(&api, 0, sizeof(api));
  api.struct_size = OPENDAL_MBT_API_V1_SIZE;
  api.requested_major = OPENDAL_MBT_ABI_V1_MAJOR;
  status = opendal_mbt_get_api(&api);
  if (status != OPENDAL_MBT_STATUS_OK) {
    (void)fprintf(stderr, "S3 example: bootstrap failed (%u)\n", status);
    return EXIT_FAILURE;
  }
  if ((api.feature_bits & OPENDAL_MBT_FEATURE_S3) == 0) {
    (void)fputs("S3 example: profile has no S3 feature; skipped\n", stdout);
    return EXIT_SUCCESS;
  }
  if (api.library_struct_size < OPENDAL_MBT_API_V1_FIELD_END(operator_s3) ||
      api.operator_s3 == NULL) {
    (void)fputs("S3 example: advertised S3 function is unavailable\n", stderr);
    return EXIT_FAILURE;
  }

  memset(&options, 0, sizeof(options));
  options.struct_size = (uint32_t)sizeof(options);
  options.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  options.present_bits = OPENDAL_MBT_S3_ENDPOINT_PRESENT;
  options.auth_kind = OPENDAL_MBT_S3_AUTH_UNSIGNED;
  options.source_kind = OPENDAL_MBT_S3_SOURCE_DEFAULT_CHAIN;
  options.bucket = text_view("moonbit-binding-example");
  options.region = text_view("us-east-1");
  options.endpoint = text_view("http://127.0.0.1:9000");

  status = api.operator_s3(&options, &operator_, &info, &error);
  if (status != OPENDAL_MBT_STATUS_OK || operator_ == NULL || info == NULL ||
      error != NULL) {
    (void)fprintf(stderr, "S3 example: typed construction failed (%u)\n",
                  status);
    goto cleanup;
  }

  memset(&info_view, 0, sizeof(info_view));
  info_view.struct_size = (uint32_t)sizeof(info_view);
  info_view.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  status = api.operator_info_view(info, &info_view);
  if (status != OPENDAL_MBT_STATUS_OK || info_view.scheme.len != UINT64_C(2) ||
      memcmp(info_view.scheme.data, "s3", 2) != 0) {
    (void)fputs("S3 example: unexpected operator info\n", stderr);
    goto cleanup;
  }

  (void)fputs("constructed unsigned S3 operator without I/O\n", stdout);
  result = EXIT_SUCCESS;

cleanup:
  if (error != NULL && api.error_free != NULL) {
    api.error_free(error);
  }
  if (info != NULL && api.operator_info_free != NULL) {
    api.operator_info_free(info);
  }
  if (operator_ != NULL && api.operator_free != NULL) {
    api.operator_free(operator_);
  }
  return result;
}
