/* Release probe: link the packaged native archive and verify its identity. */

#include "../../native/include/opendal_mbt.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int view_equals(opendal_mbt_bytes_view_v1_t view, const char *expected) {
  size_t expected_len = strlen(expected);
  if (expected_len > UINT64_MAX || view.len != (uint64_t)expected_len) {
    return 0;
  }
  if (view.len == 0) {
    return 1;
  }
  return view.data != NULL && memcmp(view.data, expected, expected_len) == 0;
}

static void print_view(FILE *stream, opendal_mbt_bytes_view_v1_t view) {
  if (view.data != NULL && view.len <= (uint64_t)SIZE_MAX) {
    (void)fwrite(view.data, 1, (size_t)view.len, stream);
  }
}

int main(int argc, char **argv) {
  opendal_mbt_api_v1_t api;
  opendal_mbt_library_info_view_v1_t info;
  opendal_mbt_status_t status;

  if (argc != 3) {
    (void)fputs(
        "usage: library-info-probe <binding-version> <service-profile>\n",
        stderr);
    return EXIT_FAILURE;
  }
  memset(&api, 0, sizeof(api));
  api.struct_size = OPENDAL_MBT_API_V1_SIZE;
  api.requested_major = OPENDAL_MBT_ABI_V1_MAJOR;
  status = opendal_mbt_get_api(&api);
  if (status != OPENDAL_MBT_STATUS_OK ||
      api.library_struct_size < OPENDAL_MBT_API_V1_FIELD_END(library_info) ||
      (api.feature_bits & OPENDAL_MBT_FEATURE_BASE) == 0 ||
      api.library_info == NULL) {
    (void)fprintf(stderr, "cannot load library_info: status=%u\n", status);
    return EXIT_FAILURE;
  }

  memset(&info, 0, sizeof(info));
  info.struct_size = (uint32_t)sizeof(info);
  info.struct_version = OPENDAL_MBT_STRUCT_VERSION_V1;
  status = api.library_info(&info);
  if (status != OPENDAL_MBT_STATUS_OK) {
    (void)fprintf(stderr, "library_info failed: status=%u\n", status);
    return EXIT_FAILURE;
  }
  if (!view_equals(info.binding_version, argv[1])) {
    (void)fputs("binding version mismatch: expected ", stderr);
    (void)fputs(argv[1], stderr);
    (void)fputs(", got ", stderr);
    print_view(stderr, info.binding_version);
    (void)fputc('\n', stderr);
    return EXIT_FAILURE;
  }
  if (!view_equals(info.service_profile, argv[2])) {
    (void)fputs("service profile mismatch: expected ", stderr);
    (void)fputs(argv[2], stderr);
    (void)fputs(", got ", stderr);
    print_view(stderr, info.service_profile);
    (void)fputc('\n', stderr);
    return EXIT_FAILURE;
  }
  (void)fputs("native library identity: ", stdout);
  print_view(stdout, info.binding_version);
  (void)fputs(" (", stdout);
  print_view(stdout, info.service_profile);
  (void)fputs(")\n", stdout);
  return EXIT_SUCCESS;
}
