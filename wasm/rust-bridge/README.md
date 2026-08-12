# OpenDAL MoonBit Wasm bridge canary

This crate is the Rust half of the first OpenDAL/MoonBit WebAssembly proof. It
builds as a core Wasm module and exposes a scalar-only ABI that a MoonBit Wasm
module can import through its host. Values cross the boundary as `u32`/`i32`
scalars and generation-checked handles; Rust pointers and language-owned
objects never cross it.

The bridge uses OpenDAL's always-available `memory` service. `write`, `read`,
and `stat` poll their OpenDAL future **exactly once**. An immediately-ready
future completes normally; a pending future returns `ASYNC_PENDING` (`9`).
This is deliberately a canary adapter, not a general browser executor. It must
not be used for OPFS, S3, or any service that can genuinely suspend. Those
services need the scheduled async protocol described in
`docs/design/wasm-integration.md`.

## Build

The expected artifact is:

```sh
CARGO_PROFILE_RELEASE_PANIC=abort cargo build \
  --manifest-path wasm/rust-bridge/Cargo.toml \
  --target wasm32-unknown-unknown \
  --release
```

The bridge is a separate Cargo crate from the native static archive, so its
minimal dependency and feature set does not change native build profiles.

## ABI conventions

- ABI version is `0x0001_0000` (major 1, minor 0).
- Handles are positive signed 32-bit values as well as valid `u32` values; the
  generation field is 15 bits so MoonBit can carry them in an `Int` unchanged.
  A slot is permanently retired when that generation space is exhausted, so a
  released handle never becomes valid again through wraparound.
- Successful constructors, `read`, `stat`, error snapshots, and error messages
  return a non-zero handle; `0` means failure or "no last error" where noted.
- Status-returning calls return `0` on success and a stable error code on
  failure.
- `buffer_len`, `buffer_get`, and `metadata_is_file` return a non-negative
  value on success and `-1` on failure.
- A failure records a sticky last error. `last_error_take` converts it to an
  owned error handle. Error-message queries return a new buffer handle, which
  the caller must release.
- `metadata_content_length_low` and `metadata_content_length_high` return the
  two halves of the unsigned 64-bit content length. Their scalar `0` is
  ambiguous, so callers should consult the sticky last error after an invalid
  metadata handle.
- Every handle returned by the bridge must be released with the matching
  release function. `live_handle_count` is the canary leak oracle.

Stable status/error codes are: `0` success/no error, `1` invalid or stale
handle, `2` wrong resource type, `3` buffer too large, `4` index out of bounds,
`5` invalid byte, `6` invalid UTF-8 path, `7` OpenDAL `NotFound`, `8` other
OpenDAL error, `9` async operation became pending, `10` handle limit, and `11`
scalar length overflow.

The exported symbols all use the `opendal_mbt_wasm_` prefix. See `src/lib.rs`
for the complete list and exact signatures.
