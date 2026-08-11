# Native Distribution Contract

## Goal

`eric-song-nop/opendal` should behave like an ordinary native Moon package.
A consumer installs it and imports it in the usual way:

```sh
moon add eric-song-nop/opendal
moon run --target native cmd/main
```

The consumer must not install Rust, clone this repository, set
`LIBRARY_PATH`, or duplicate linker flags in its own `moon.pkg`. The package
owns the complete native dependency setup.

Moon currently has no stable declarative mechanism for attaching a native
archive to a registry package. Until one exists, this module uses Moon's
experimental `--moonbit-unstable-prebuild` configuration script, following
the same model as `moonbitlang/llvm.mbt`. The script requires Node.js 18 or
newer. That runtime requirement is temporary toolchain debt, not part of the
OpenDAL API.

## Supported release matrix

The first prebuilt profile is `local`:

| Profile | OpenDAL services | Rust features |
| --- | --- | --- |
| `local` | `memory`, `fs` | `blocking`, `services-fs` |

The first host matrix is intentionally small:

| Host | Release target | Compatibility floor |
| --- | --- | --- |
| Apple silicon macOS | `aarch64-apple-darwin` | macOS 11.0 |
| x86-64 Linux | `x86_64-unknown-linux-gnu` | glibc 2.35 |

The module and package declare native-only support. Unsupported hosts fail
before downloading with a message listing the supported matrix. Windows,
Intel macOS, Linux arm64, and musl require separately built and tested
artifacts; the installer never guesses that one target is compatible with
another.

## Published artifact

Every supported target has one archive named:

```text
opendal-mbt-<binding-version>-<artifact-revision>-<profile>-<rust-target>.tar.gz
```

The archive contains only runtime distribution material:

```text
manifest.json
LICENSE
lib/libopendal_mbt_native.a
```

`manifest.json` uses schema version 1 and records:

- the binding, ABI, OpenDAL, and artifact revision versions;
- the service profile and enabled services;
- the exact Rust target and its compatibility floor;
- the static-library relative path, byte size, and SHA-256;
- the exact system libraries reported by
  `cargo rustc -- --print native-static-libs` on that target.

The release workflow produces archives with normalized ordering, owner,
permissions, and timestamps. It builds each target on that target rather than
assuming that one host's native-link report applies to another host.

## Trust and cache model

The module source pins each archive URL, archive size and SHA-256, extracted
library size and SHA-256, manifest identity, compatibility floor, and system
link flags. A checksum downloaded beside a mutable archive is useful for
humans but is not the installer's trust root.

The configuration script selects the current host and installs the matching
archive below:

```text
$MOON_HOME/cache/lib/opendal-mbt/
  <binding-version>/<profile>/<rust-target>/sha256-<archive-digest>/
```

A cold cache performs an HTTPS download, checks the archive before extraction,
checks the manifest and static library after extraction, and atomically moves
the completed directory into place. A lock directory coordinates concurrent
Moon processes. Partial, stale, or invalid entries are never treated as a
cache hit.

A hot cache performs no network request and revalidates the completion marker,
manifest, library size, and library digest. This makes ordinary offline builds
work after the first successful install.

The script writes diagnostics to stderr because stdout is the Moon JSON
protocol. Its result contributes the exact absolute archive path and target
system libraries to the link configuration for `eric-song-nop/opendal`.

## Maintainer path

Repository development still builds the Rust archive from source. The
Makefile passes the exact locally built archive to the configuration script
through a maintainer-only override, so Moon tests exercise uncommitted Rust
changes instead of silently using a released binary. This override is not
needed or documented in the consumer quickstart.

Source builds are also the fallback for maintainers adding a target or
service profile. They are not an automatic fallback for consumers: silently
invoking Cargo would violate the ordinary-package contract and make builds
depend on an unpinned host toolchain.

## Release and acceptance gates

A release is eligible only when all of the following pass:

1. Rust, C ABI, MoonBit, debug/release, sanitizer, and public-interface tests.
2. Reproducible artifact generation on every advertised host.
3. Resolver tests for target selection, integrity rejection, cache recovery,
   concurrency, and offline hot-cache use.
4. A consumer assembled from the `moon package` archive that has no repository
   checkout, Cargo/Rust on `PATH`, `LIBRARY_PATH`, or consumer linker flags.
5. That consumer runs the memory and filesystem workflows with ordinary
   `moon test --target native` commands on every advertised host.

The release workflow publishes the two archives under the package version
tag. Adding a target means building it in CI, recording its own compatibility
floor and native link flags, extending the pinned artifact table, and adding
the same clean-consumer gate for that host.
