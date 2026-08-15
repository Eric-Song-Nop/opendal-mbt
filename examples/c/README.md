# C ABI examples

This directory contains two direct consumers of the versioned OpenDAL MoonBit
C ABI. They validate the public header independently of MoonBit and use only
the negotiated function-table prefix supported by the loaded library.

## Memory round trip

[`memory.c`](memory.c) exercises the local data and ownership path. It:

- negotiates ABI major v1 and checks both feature bits and each function's
  `OPENDAL_MBT_API_V1_FIELD_END` boundary;
- constructs an in-memory operator and inspects its owned `OperatorInfo`;
- deliberately reads a missing object to inspect and free an owned error;
- writes and reads binary data containing embedded NUL and non-UTF-8 bytes;
- reads the same object through bounded `READ_STREAM` chunks and checks stable
  end-of-stream behavior;
- inspects and frees write metadata;
- uses `buffer_len` plus both phases of `buffer_copy`, including an untouched
  destination-tail canary; and
- releases every Rust-owned handle, snapshot, error, and buffer through its
  paired ABI function.

The program prints one **expected** not-found error before reporting a
successful binary round trip. Any unexpected transport status, missing output,
ABI field, invalid borrowed view, ownership inconsistency, or byte mismatch
causes a nonzero exit.

## Typed S3 construction

[`s3.c`](s3.c) exercises the append-only S3 ABI group without making network
requests. It:

- negotiates ABI major v1 and verifies both the S3 feature bit and the
  `operator_s3` function-table boundary;
- fills a versioned `opendal_mbt_s3_options_v1_t` with an unsigned auth policy,
  bucket, region, and local example endpoint;
- constructs the typed S3 Operator and verifies its owned `OperatorInfo`
  reports scheme `s3`; and
- frees the Operator, info snapshot, and any owned error through their paired
  functions.

With the `standard` profile it prints
`constructed unsigned S3 operator without I/O`. A `local` profile does not
advertise S3, so the example reports that it was skipped and exits successfully.
The endpoint is illustrative and no server or credentials are needed because
Operator construction performs no object I/O.

## Build and run

Run the warning-clean header/syntax check without a native library:

```sh
make -C examples/c syntax
```

Build the bridge and ask `rustc` for the platform-specific native libraries:

```sh
cargo build --release --locked
cargo rustc -p opendal-mbt-native --release -- --print native-static-libs
```

Then pass the bridge artifact and the printed libraries explicitly. The `run`
target builds and executes both `memory` and `s3`:

```sh
make -C examples/c run \
  OPENDAL_MBT_LIB="$PWD/target/release/libopendal_mbt_native.a" \
  OPENDAL_MBT_NATIVE_LIBS='native libraries printed by rustc'
```

`OPENDAL_MBT_LIB` may also name the platform's shared-library artifact. A Rust
`staticlib` can require additional platform libraries; use the exact
`native-static-libs` output produced by the bridge build instead of copying a
list from another operating system. The main native build should publish both
the artifact path and that linker output.

The program prints one **expected** not-found error before reporting a
successful binary round trip, followed by either successful S3 construction or
the profile-honest S3 skip message.
