/* Staticlib-host regression probe for the async completion pipe contract. */

/* Expose sigaction under strict C11 builds on glibc. */
#define _POSIX_C_SOURCE 200809L

#include "../../native/include/opendal_mbt.h"

#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static opendal_mbt_bytes_view_v1_t bytes_view(const char *value) {
  opendal_mbt_bytes_view_v1_t view;
  view.data = (const uint8_t *)value;
  view.len = (uint64_t)strlen(value);
  return view;
}

static int nonblocking_pipe(int fds[2]) {
  int flags;
  if (pipe(fds) != 0) {
    return 0;
  }
  flags = fcntl(fds[1], F_GETFL);
  if (flags == -1 || fcntl(fds[1], F_SETFL, flags | O_NONBLOCK) == -1) {
    (void)close(fds[0]);
    (void)close(fds[1]);
    return 0;
  }
  return 1;
}

static int start_pending_read(const opendal_mbt_api_v1_t *api,
                              opendal_mbt_operator_v1_t *operator_,
                              int completion_fd) {
  opendal_mbt_async_task_v1_t *task = NULL;
  opendal_mbt_error_v1_t *error = NULL;
  opendal_mbt_bytes_view_v1_t path = bytes_view("completion-missing");
  opendal_mbt_status_t status = api->async_operator_read_start(
      operator_, &path, NULL, api->max_output_bytes, completion_fd, &task,
      &error);
  if (status != OPENDAL_MBT_STATUS_OK || task == NULL || error != NULL) {
    if (error != NULL) {
      api->error_free(error);
    }
    return 0;
  }
  api->async_task_cancel(task);
  api->async_task_free(task);
  return 1;
}

int main(void) {
  opendal_mbt_api_v1_t api;
  opendal_mbt_operator_v1_t *operator_ = NULL;
  opendal_mbt_operator_info_v1_t *info = NULL;
  opendal_mbt_error_v1_t *error = NULL;
  opendal_mbt_bytes_view_v1_t scheme = bytes_view("memory");
  opendal_mbt_status_t status;
  int blocking[2];
  int closed_reader[2];
  int ignored_reader[2];
  int full[2];
  uint8_t fill[4096];

  (void)alarm(5U);
  memset(&api, 0, sizeof(api));
  api.struct_size = OPENDAL_MBT_API_V1_SIZE;
  api.requested_major = OPENDAL_MBT_ABI_V1_MAJOR;
  status = opendal_mbt_get_api(&api);
  if (status != OPENDAL_MBT_STATUS_OK ||
      (api.feature_bits & OPENDAL_MBT_FEATURE_ASYNC) == 0) {
    return EXIT_FAILURE;
  }
  status = api.operator_new(&scheme, NULL, 0, &operator_, &info, &error);
  if (status != OPENDAL_MBT_STATUS_OK || operator_ == NULL || info == NULL ||
      error != NULL) {
    return EXIT_FAILURE;
  }

  if (pipe(blocking) != 0) {
    return EXIT_FAILURE;
  }
  {
    opendal_mbt_async_task_v1_t *task = NULL;
    opendal_mbt_bytes_view_v1_t path = bytes_view("completion-blocking");
    status = api.async_operator_read_start(operator_, &path, NULL,
                                           api.max_output_bytes, blocking[1],
                                           &task, &error);
    if (status != OPENDAL_MBT_STATUS_ERROR || task != NULL || error == NULL) {
      return EXIT_FAILURE;
    }
    api.error_free(error);
    error = NULL;
  }
  (void)close(blocking[0]);
  (void)close(blocking[1]);

  if (!nonblocking_pipe(closed_reader)) {
    return EXIT_FAILURE;
  }
  (void)close(closed_reader[0]);
  if (signal(SIGPIPE, SIG_DFL) == SIG_ERR ||
      !start_pending_read(&api, operator_, closed_reader[1])) {
    return EXIT_FAILURE;
  }
  (void)close(closed_reader[1]);
  {
    struct sigaction action;
    if (sigaction(SIGPIPE, NULL, &action) != 0 ||
        action.sa_handler != SIG_DFL) {
      return EXIT_FAILURE;
    }
  }

  if (!nonblocking_pipe(ignored_reader)) {
    return EXIT_FAILURE;
  }
  (void)close(ignored_reader[0]);
  if (signal(SIGPIPE, SIG_IGN) == SIG_ERR ||
      !start_pending_read(&api, operator_, ignored_reader[1])) {
    return EXIT_FAILURE;
  }
  (void)close(ignored_reader[1]);
  {
    struct sigaction action;
    if (sigaction(SIGPIPE, NULL, &action) != 0 ||
        action.sa_handler != SIG_IGN) {
      return EXIT_FAILURE;
    }
  }

  if (!nonblocking_pipe(full)) {
    return EXIT_FAILURE;
  }
  memset(fill, 0xa5, sizeof(fill));
  while (write(full[1], fill, sizeof(fill)) >= 0) {
  }
  if (errno != EAGAIN || !start_pending_read(&api, operator_, full[1])) {
    return EXIT_FAILURE;
  }
  (void)close(full[0]);
  (void)close(full[1]);

  api.operator_info_free(info);
  api.operator_free(operator_);
  return EXIT_SUCCESS;
}
