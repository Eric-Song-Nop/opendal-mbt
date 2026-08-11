# OpenDAL for MoonBit

Safe, checked MoonBit bindings for Apache OpenDAL. The current release supports
the native backend and ships the memory and filesystem service profiles.

Build the pinned Rust static library before running MoonBit code:

```sh
OPENDAL_MBT_SOURCE=/path/to/unpacked/opendal-mbt
cargo build --manifest-path "$OPENDAL_MBT_SOURCE/Cargo.toml" --workspace --locked
LIBRARY_PATH="$OPENDAL_MBT_SOURCE/target/debug" \
  moon test --target native --frozen
```

The current release is source-built. Prebuilt native artifacts and automatic
installation are deferred until the distribution contract is designed. A
`moon add` alone therefore does not build the required Rust archive: keep a
source checkout or unpacked source release available for the build above.

The final native package or executable also links that archive. Add the
following entry to its `moon.pkg`, and point `LIBRARY_PATH` at the matching
Cargo profile directory when invoking `moon`:

```moonbit nocheck
options(
  link: { "native": { "cc-link-flags": "-lopendal_mbt_native -lm" } },
)
```

## Whole-object storage

Inputs accept StringView and BytesView, while returned data is owned.

```mbt check
///|
test "README quickstart" {
  let operator = @opendal.Operator::new("memory")
  let storage = b"__hello from MoonBit__"
  let payload : BytesView = storage[2:20]
  operator.write("hello.txt", payload) |> ignore
  assert_eq(operator.read("hello.txt"), b"hello from MoonBit")
}
```

Range reads use an explicit algebraic value. An opened Reader is random access:
each call supplies its own range and does not advance a hidden cursor.

```mbt check
///|
test "README random reader" {
  let operator = @opendal.Operator::new("memory")
  operator.write("archive.bin", b"0123456789") |> ignore
  assert_eq(
    operator.read("archive.bin", range=Range(offset=2UL, length=4UL)),
    b"2345",
  )

  let reader = operator.open_reader("archive.bin")
  assert_eq(reader.read(From(offset=6UL)), b"6789")
  assert_eq(reader.read(Suffix(length=3UL)), b"789")
  reader.close()
}
```

Pass backend configuration with the optional labelled config argument:

```mbt check
///|
test "README configured operator" {
  let operator = @opendal.Operator::new("memory", config={ "root": "/example/" })
  assert_eq(operator.info().root, "/example/")
}
```

## Eager and streaming listing

Operation options are direct optional labelled arguments. Use list when all
entries should be materialized, or open_lister for a fallible cursor.

```mbt check
///|
test "README listing" {
  let operator = @opendal.Operator::new("memory")
  operator.write("logs/2026/one.txt", b"one") |> ignore
  operator.write("logs/2026/two.txt", b"two") |> ignore
  let entries = operator.list("logs/", recursive=true)
  assert_true(entries.length() >= 2)

  let lister = operator.open_lister("logs/", recursive=true)
  let mut count = 0
  for ;; {
    match lister.next() {
      Some(_) => count = count + 1
      None => break
    }
  }
  lister.close()
  assert_true(count >= 2)
}
```

## Chunked writes

An opened Writer accepts complete chunks and reports success only after an
explicit finish. Dropping an unfinished Writer does not finish it implicitly.

```mbt check
///|
test "README chunked writer" {
  let operator = @opendal.Operator::new("memory")
  let writer = operator.open_writer(
    "upload.bin",
    content_type="application/octet-stream",
  )
  writer.write(b"hello ")
  writer.write(b"from MoonBit")
  let metadata = writer.finish()
  assert_eq(metadata.content_length, 18UL)
  assert_eq(operator.read("upload.bin"), b"hello from MoonBit")
}
```

## Checked errors

Storage failures use OpenDalError rather than stringly typed Result values.

```mbt check
///|
test "README checked errors" {
  let operator = @opendal.Operator::new("memory")
  try operator.read("missing.txt") catch {
    error => {
      assert_true(error.kind() is NotFound)
      assert_true(error.info().operation is Read)
      assert_false(error.is_temporary())
      assert_true("\{error}".contains("OpenDAL NotFound"))
    }
  } noraise {
    _ => fail("expected missing read to raise")
  }
}
```

The generated pkg.generated.mbti file is the authoritative public surface.
Copy and rename will be added only when their complete MoonBit, C, and Rust
vertical slices are implemented.
