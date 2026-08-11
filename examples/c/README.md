# Memory service C example

`memory.c` is a real consumer of the versioned OpenDAL MoonBit C ABI. It:

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

Run the warning-clean header/syntax check without a native library:

```sh
make -C examples/c syntax
```

Build the bridge and ask `rustc` for the platform-specific native libraries:

```sh
cargo build --release --locked
cargo rustc -p opendal-mbt-native --release -- --print native-static-libs
```

Then pass the bridge artifact and the printed libraries explicitly:

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
successful binary round trip. Any unexpected transport status, missing output,
ABI field, invalid borrowed view, ownership inconsistency, or byte mismatch
causes a nonzero exit.
