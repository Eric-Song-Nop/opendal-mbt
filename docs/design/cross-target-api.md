# Cross-target MoonBit API contract

The native and browser Wasm packages expose one backend-neutral OpenDAL
domain model. They intentionally use different scheduling adapters:

- native `AsyncOperator` methods are ordinary MoonBit `async fn`s;
- browser Wasm methods end in `_callback`, deliver `Result` through the final
  required `callback~` label, and return a cancellable `Task`.

The machine-readable contract in `cross-target-api.json` normalizes those two
forms and freezes the shared whole-object surface: `check`, `exists`, `read`,
`stat`, `write`, `create_dir`, `delete`, `list`, `copy`, and `rename`. It also
requires the shared value and error shapes to remain aligned. A runtime
artifact may support only the schemes compiled into its service profile;
scheme selection never changes this public API.

The following differences are deliberate extensions rather than parity gaps:

- Wasm operators and tasks have explicit idempotent `close`/`cancel`
  lifecycles because their resources cross two Wasm instances.
- Native exposes streaming Reader/Writer resources backed by its stable async
  runtime. Browser streaming remains separate work until its backpressure and
  continuation contract is stable.
- Backend capability errors remain observable. In particular, copy uses the
  backend's native copy operation and is never silently emulated as read plus
  write.

Run `python3 scripts/check-cross-target-api.py` after regenerating both
`pkg.generated.mbti` snapshots. The checker reports the exact operation,
shared type, or method set that drifted.
