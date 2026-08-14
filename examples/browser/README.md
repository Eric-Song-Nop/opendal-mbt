# Portable async example: native and browser

This example runs shared `Eric-Song-Nop/opendal` async code on both the native
backend and inside a real Chrome or Chromium process. The portable application
uses the memory service to exercise direct write/read, a streaming writer, a
streaming reader, and explicit resource closure. A second shared function makes
a delayed S3 read and proves that pending OpenDAL I/O does not block MoonBit's
async scheduler.

From the repository root, run either backend in one command:

```sh
moon -C examples/browser run --target native --release .
```

```sh
moon -C examples/browser run --target js --release .
```

The direct native command prints the portable application marker. The browser
command also runs the delayed-I/O proof and prints both markers:

```text
OPENDAL_PORTABLE_ASYNC_OK: 24 streamed bytes, scheduler=2
OPENDAL_ASYNC_IO_NONBLOCKING_OK: heartbeat ran before delayed S3 I/O completed
```

## Requirements

- Both commands: MoonBit
- JavaScript/browser command: Node.js 18 or newer and Chrome or Chromium
- Native command: a released matching native artifact, or the repository's
  maintainer library while `0.2.0` remains unpublished

For the unreleased source tree, maintainers can build and select that library
before running the native command:

```sh
make native
OPENDAL_MBT_NATIVE_LIB="$PWD/target/debug/libopendal_mbt_native.a" \
  OPENDAL_MBT_SOURCE_PROFILE=standard \
  moon -C examples/browser run --target native --release .
```

After the corresponding release artifact exists, the single native `moon`
command shown above resolves it automatically.

The JavaScript command launches a real browser; it does not run the OpenDAL
operations under Node. The launcher discovers common Chrome and Chromium
installations on macOS, Linux, and Windows. If yours is elsewhere, set
`OPENDAL_MBT_BROWSER_BIN` to the browser executable:

```sh
OPENDAL_MBT_BROWSER_BIN=/path/to/chrome \
  moon -C examples/browser run --target js --release .
```

`run.mbt` is the application-facing program compiled unchanged for both
targets. `nonblocking.mbt` contains the delayed-I/O verification function used
unchanged by native and JavaScript. The target-specific `main.*.mbt` files only
select the local fixture and execution host. `launcher.mbt` is a small local
host and cross-platform launcher that keeps the browser path to one command.
The embedded OpenDAL runtime means the JavaScript command does not require
Rust, `wasm-bindgen`, npm, or a bundler.

The `scheduler=2` field comes from two deterministic tasks that each yield and
resume. It confirms the MoonBit async scheduler is active on each backend; it
does **not** claim that fast in-memory I/O is a latency or non-blocking
benchmark.

This shared example deliberately uses only the portable async API. Operations
such as eager listing and deletion are currently browser extensions, so adding
them here would make the native and browser programs differ.

## Non-blocking I/O verification

The JavaScript command automatically starts two independent local HTTP origins:
one serves the application and the other acts as a delayed S3 endpoint. Chrome
must complete a real CORS preflight and send SigV4 authorization headers to the
exact fixture path. The fixture waits 750 ms before returning a valid S3
`NoSuchKey` XML response, which OpenDAL must expose as `NotFound`.

The direct native example intentionally has no Python dependency. Maintainers
can run the same `nonblocking.mbt` proof against a native local fixture with
Python 3:

```sh
python3 examples/browser/verify_native_nonblocking.py
```

On an unreleased source checkout, run `make native` first so the probe can use
the local standard-profile library. Released consumers obtain the matching
native artifact through the normal package prebuild.

Before either S3 read begins, the MoonBit function waits until a sibling task
has armed a 50 ms heartbeat. It then checks that the heartbeat ran before the
read completed and that the read failed specifically with `NotFound`. Each
fixture holds an accepted GET for 750 ms. A synchronous wrapper would block the
scheduler and fail this ordering check. On the browser path, the separate
fixture independently requires the expected CORS flow, request path, and
SigV4-shaped headers before returning the valid `NoSuchKey` response, so those
protocol-shape and response-decoding failures cannot masquerade as success. It
does not recompute the signature HMAC. No wall-clock performance threshold is
used.

That ordering is the black-box scheduler proof on both backends. The native
implementation path is also genuinely asynchronous: MoonBit starts an async
ABI task and waits on a nonblocking pipe; Rust retains OpenDAL's async Operator
and awaits its native reader and writer futures rather than invoking the
blocking facade on a worker thread. The native ABI tests separately reject a
blocking completion descriptor and exercise full-pipe and closed-reader
wakeups.

In a source checkout, the nested `moon.work` resolves
`Eric-Song-Nop/opendal` to the enclosing module. Moon excludes workspace files
from published archives, but ships the rest of this Browser example; after the
corresponding release exists, its pinned import resolves normally from the
registry.
