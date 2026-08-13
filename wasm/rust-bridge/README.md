# OpenDAL MoonBit Wasm bridge canary

This crate is the Rust half of the OpenDAL/MoonBit WebAssembly proof. It builds
as a core Wasm module and exposes a scalar-only resource/task ABI that a
MoonBit Wasm module imports through the repository loader. Values cross the
Rust boundary as fixed-width integer scalars and generation-checked handles;
Rust pointers and language-owned objects never cross it.

The bridge uses OpenDAL's always-available `memory` service and currently has
two execution paths:

- synchronous `write`, `read`, and `stat` poll exactly once and exist only for
  the Node ABI smoke test; a pending future returns `ASYNC_PENDING` (`9`);
- `*_start` creates an owned task driven by
  `wasm_bindgen_futures::spawn_local`. The canary wraps every task in a
  zero-delay timer so its first poll is observably `Pending` before it can
  become ready.

The task path is exercised in real Chrome/Chromium. This artifact currently
compiles only the deterministic memory fixture, but construction is routed
through OpenDAL's generic service registry so other browser-compatible
profiles do not require backend-specific facade APIs.

## Build

The repository command builds the raw module, runs the exactly pinned
wasm-bindgen CLI, and emits the matching `.mjs` and processed `_bg.wasm` pair:

```sh
make wasm-rust
```

The raw Cargo input to wasm-bindgen can also be built directly:

```sh
CARGO_PROFILE_RELEASE_PANIC=abort cargo build --locked \
  --package opendal-mbt-wasm-bridge \
  --target wasm32-unknown-unknown \
  --release
```

The bridge is a separate Cargo crate from the native static archive, so its
minimal dependency and feature set does not change native build profiles.

## ABI conventions

- ABI version is `0x0001_0002` (major 1, minor 2).
- Feature flags currently equal `0x0000_007f`: bit 0 memory service, bit 1
  poll-once canary, bit 2 generation handles, bit 3 binary buffers, and bit 4
  task ABI, bit 5 generic operator construction and service inspection, and
  bit 6 common create-dir/delete mutations.
- Handles are positive signed 32-bit values as well as valid `u32` values; the
  generation field is 15 bits so MoonBit can carry them in an `Int` unchanged.
  A slot is permanently retired when that generation space is exhausted, so a
  released handle never becomes valid again through wraparound.
- Successful constructors, synchronous `read`/`stat`, task starts, task take,
  result take, error snapshots, and error messages return a non-zero handle;
  `0` means failure or "no last error" where noted.
- Status-returning calls return `0` on success and a stable error code on
  failure.
- `buffer_len`, `buffer_get`, and `metadata_is_file` return a non-negative
  value on success and `-1` on failure.
- Synchronous failures, task-start failures, and ABI misuse record a sticky
  last error. `last_error_take` converts it to an owned error handle. A failed
  asynchronous OpenDAL operation instead owns its error inside its completion;
  `completion_take_error` moves it to an error handle. Error-message queries
  return a new buffer handle, which the caller must release.
- `metadata_content_length_low` and `metadata_content_length_high` return the
  two halves of the unsigned 64-bit content length. Their scalar `0` is
  ambiguous, so callers should consult the sticky last error after an invalid
  metadata handle.
- Every handle returned by the bridge must be released with the matching
  release function. Releasing a task or completion drops any result it still
  owns. `live_handle_count` is the canary leak oracle.

Task state scalars are `1` pending, `2` ready, `3` cancelled, and `4`
consumed. Completion kinds are `1` write, `2` read, `3` stat, `4` create-dir,
and `5` delete. `task_take` moves a ready result into a separately owned
completion exactly once. Read, metadata, and error take operations then consume
that completion; successful unit completions are explicitly released.

Task cancellation is **logical**. Cancelling pending work makes late completion
inert; cancelling ready work wins over an unconsumed result. It does not abort
the underlying OpenDAL future. `opendal_mbt_wasm_teardown` is idempotent and
permanent for the instance: it clears every live resource and ignores late
task completion.

Stable status/error codes are: `0` success/no error, `1` invalid or stale
handle, `2` wrong resource type, `3` buffer too large, `4` index out of bounds,
`5` invalid byte, `6` invalid UTF-8 path, `7` OpenDAL `NotFound`, `8` other
OpenDAL error, `9` async operation became pending, `10` handle limit, and `11`
scalar length overflow, `12` task not ready, `13` task already consumed, and
`14` bridge instance torn down.

The exported symbols all use the `opendal_mbt_wasm_` prefix. See `src/lib.rs`
for the complete list and exact signatures.
