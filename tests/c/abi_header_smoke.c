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
ABI_STATIC_ASSERT(sizeof(opendal_mbt_bool_t) == 4,
                  "ABI boolean must be 32-bit");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_byte_range_v1_t) == 32,
                  "byte range layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_timestamp_v1_t) == 16,
                  "timestamp layout drifted");
ABI_STATIC_ASSERT(sizeof(opendal_mbt_capability_v1_t) == 32,
                  "capability layout drifted");
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
ABI_STATIC_ASSERT(OPENDAL_MBT_API_V1_FIELD_END(library_info) <=
                      OPENDAL_MBT_API_V1_FIELD_END(error_view),
                  "function table order drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_READ_OPTIONS_V1_MIN_SIZE <=
                      sizeof(opendal_mbt_read_options_v1_t),
                  "read options v1.0 prefix drifted");
ABI_STATIC_ASSERT(OPENDAL_MBT_READER_OPTIONS_V1_MIN_SIZE <=
                      sizeof(opendal_mbt_reader_options_v1_t),
                  "reader options v1.0 prefix drifted");
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
