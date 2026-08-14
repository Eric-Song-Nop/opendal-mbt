# OpenDAL MoonBit Wasm bridge

This crate is the Rust half of the OpenDAL/MoonBit WebAssembly binding. It
builds as a core Wasm module and exposes a scalar resource/task ABI imported by
the MoonBit Wasm facade through the repository loader. Values cross the Rust
boundary as fixed-width scalars and generation-checked handles; Rust pointers
and language-owned objects never cross as public ABI values.

The bridge initializes OpenDAL's compiled service registry. The public facade
uses one backend-neutral construction path,
`Operator::new(scheme, config)`, for every service present in the selected
artifact. The current browser-memory artifact compiles only the deterministic
memory fixture. Enabling another browser-compatible service is an artifact
profile and acceptance-test change, not a new facade constructor.

## Execution paths

The public MoonBit facade imports only task-start operations for create-dir,
write, read, stat, bounded list, and delete. Each start clones the OpenDAL
operator, copies its inputs, publishes a task handle, and schedules the future
with `wasm_bindgen_futures::spawn_local`.

Production tasks await the OpenDAL future directly. A raw canary setter can
wrap subsequently started tasks in a zero-delay future that guarantees an
observable first `Pending` poll. This hook is disabled by default and is used
only by the Chrome scheduling test.

The bridge still exports synchronous poll-once operations and canary
diagnostics as low-level fixture/ABI oracles. They are not imported or exposed
by the public MoonBit package and must not be treated as browser storage APIs.

## Build

The repository command builds the raw module, runs the exactly pinned
wasm-bindgen CLI, and emits the matching `.mjs` and processed `_bg.wasm` pair:

```sh
make wasm-rust
```

The raw Cargo input can also be built directly:

```sh
CARGO_PROFILE_RELEASE_PANIC=abort cargo build --locked \
  --package opendal-mbt-wasm-bridge \
  --target wasm32-unknown-unknown \
  --release
```

The bridge is separate from the native static archive, so its dependency and
feature profile does not alter the native package.

## ABI conventions

- ABI version is `0x0001_0006` (major 1, minor 6).
- The bridge reports feature flags `0x0000_07ff`: bit 0 memory fixture, bit 1
  poll-once fixture, bit 2 generation handles, bit 3 binary buffers, bit 4
  task ABI, bit 5 generic operator construction and inspection, bit 6 common
  create-dir/delete mutations, bit 7 bounded streaming list materialization,
  bit 8 bounded cross-memory transfer, bit 9 structured error snapshots, and
  bit 10 metadata snapshots plus operation options.
- The public facade requires `0x0000_07fc`; memory and poll-once are not generic
  facade requirements.
- Handles are positive signed 32-bit values as well as valid `u32` values. The
  generation field is 15 bits so MoonBit can carry a handle in `Int` without
  changing it. Slots retire before generation wraparound.
- Constructors, task starts, typed result takes, error snapshots, and message
  copies return non-zero handles. `0` indicates failure or no last error where
  documented. Status calls return `0` on success.
- Operator configuration is capped at 1,024 entries and 1 MiB of combined key
  and value UTF-8. ASCII-case-insensitive duplicate keys fail atomically.
- Every owned handle has a matching release operation. Releasing a task or
  completion drops an unconsumed result. The safe MoonBit wrappers make close
  and cancellation idempotent.
- `buffer_new_sized` allocates at most 64 MiB. `buffer_data_ptr` exposes a
  checked window of at most 256 KiB for one immediate synchronous host copy;
  the pointer is invalid after the next mutation or release of that buffer.
- Asynchronous OpenDAL errors belong to their completion. Start failures use
  the bridge's sticky last-error slot only until `last_error_take` moves the
  error into an owned handle. MoonBit then consumes a structured snapshot and
  constructs an immutable `OpenDalError`.
- Lists publish atomically only at or below 65,536 entries and 16 MiB of
  combined path, name, and pre-encoded `ODM1` metadata bytes. A backend request
  `limit` remains a service hint and does not replace those binding limits.

Task states are `1` pending, `2` ready, `3` cancelled, and `4` consumed.
Completion kinds are `1` write, `2` read, `3` stat, `4` create-dir, `5`
delete, and `6` list. `task_take` moves a ready completion exactly once.

Cancellation is logical. Cancelling pending work suppresses publication to the
MoonBit callback path; cancelling a ready task drops its unconsumed result. It
does not claim to abort an underlying browser operation. Bridge teardown is
idempotent and permanent for the instance: it clears all live resources and
ignores late completion.

Stable status/error codes are: `0` success/no error, `1` invalid or stale
handle, `2` wrong resource type, `3` buffer too large, `4` index out of bounds,
`5` invalid byte, `6` invalid UTF-8 path, `7` OpenDAL `NotFound`, `8` other
OpenDAL error, `9` poll-once operation became pending, `10` handle limit, `11`
scalar length overflow, `12` task not ready, `13` task already consumed, `14`
bridge torn down, `15` list materialization limit, `16` allocation failure,
and `17` invalid scalar argument.

Those scalar codes remain as a legacy low-level transport. Structured error
snapshots preserve the stable OpenDAL kinds `1..12`, binding kinds
`0x1001..0x1004`, statuses `1` permanent, `2` temporary, and `3` persistent,
plus the OpenDAL kind name and diagnostic message. Unknown kind or status
values fail open as `UnknownKind(code, name)` and `UnknownStatus(code)`.

ABI 1.5 added `opendal_mbt_wasm_error_snapshot_take`. Its versioned
little-endian payload is `ODE1`, schema `1`, kind, status, kind-name length,
message length, then strict UTF-8 kind-name and message bytes. A successful
snapshot consumes the error handle; allocation or encoding failure leaves it
intact.

## Metadata and operation options

ABI 1.6 adds `operator_{read,stat,write}_options_start_v1`,
`completion_take_metadata_snapshot`, and `entry_list_metadata_snapshot`. Every
path, payload, version, condition, and content-header buffer is copied before
the start call returns. Optional handle `0` means `None`; a non-zero empty
buffer means `Some("")`. Append accepts only scalar `0` or `1`, and range
scalars reject non-canonical combinations and offset-plus-length overflow.

Read accepts full, from-offset, offset-plus-length, and suffix ranges plus
version and conditional values. Suffix reads require
`base_service().capability_dyn().read_with_suffix`; the facade capability bit
and start guard deliberately ignore support synthesized by completion layers.
Stat accepts version and conditions. Write accepts append, content type,
content disposition, content encoding, cache control, and conditions, and a
successful write completion owns the returned OpenDAL `Metadata`.

Metadata uses a versioned little-endian `ODM1` snapshot with this fixed
schema-1 layout:

| Offset | Field |
| --- | --- |
| `0..4` | magic `ODM1` |
| `4..8` | schema `u32` (`1`) |
| `8..16` | presence bits `u64` |
| `16..20` | mode `u32` (`0` unknown, `1` file, `2` directory) |
| `20..24` | current flag `u32` |
| `24..28` | deleted flag `u32` |
| `28..32` | reserved zero |
| `32..40` | content length `u64` |
| `40..48` | last-modified Unix seconds `i64` |
| `48..52` | last-modified nanoseconds `u32` |
| `52..56` | reserved zero |
| `56..84` | seven `u32` string lengths |
| `84..` | concatenated strict UTF-8 payload |

Presence bits are current, last-modified, cache control, content disposition,
content encoding, content MD5, content type, ETag, and version in bits 0
through 8. String lengths and payload order are cache control, content
disposition, content encoding, content MD5, content type, ETag, and version.
Absent values have canonical zero fields, booleans are `0` or `1`, and
nanoseconds are below 1,000,000,000. Unknown bits, non-canonical fields,
malformed lengths, trailing data, invalid UTF-8, or snapshots above 64 MiB are
ABI mismatches.

Taking a completion metadata snapshot is failure-atomic: encoding or handle
capacity failure leaves the completion available for another take. A
successful take consumes it. Each list entry owns one pre-encoded, single-take
snapshot; the metadata can be partial because it is the lister's metadata and
the bridge does not issue an extra stat. The list remains releasable after a
failed or successful entry snapshot take. Legacy stat metadata-handle exports
remain as append-only ABI compatibility operations; the public facade uses
`ODM1` for stat and write.

All exported symbols use the `opendal_mbt_wasm_` prefix. The committed static
browser-memory contract records their exact current set, the module imports,
memory shape, and size ceilings. Raw fixture exports can remain in the bridge
without becoming part of the public MoonBit interface.
