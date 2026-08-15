# Native Distribution Contract

## Goal

`Eric-Song-Nop/opendal` should behave like an ordinary native Moon package.
A consumer installs it and imports it in the usual way:

```sh
moon add Eric-Song-Nop/opendal
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

## Service profiles and package selection

The published `v0.1.x` profile is immutable:

| Profile | OpenDAL services | Rust features |
| --- | --- | --- |
| `local` | `memory`, `fs` | `blocking`, `services-fs` |

Phase 5 adds one successor profile instead of a matrix of user-selectable
variants:

| Profile | OpenDAL services | Additional OpenDAL features |
| --- | --- | --- |
| `standard` | `memory`, `fs`, `s3` | `services-s3`, `http-transport-reqwest`, `http-transport-reqwest-rustls`, `executors-tokio`, `layers-retry`, `layers-timeout`, `layers-concurrent-limit` |

`standard` includes the local services, so the root Moon facade needs exactly
one native archive on each host. There is no public profile selector and no
environment variable that changes the archive. The published `v0.1.0` package
selected `artifacts.json`. The `v0.2.0` release atomically selects
`artifacts-standard.json`, with one pinned target-native archive for each of
the three advertised hosts.

The local `v0.1.0-r1` records and their digests are never rewritten. Local and
standard pins live in separate tables, and their cache paths include the
profile name, so an existing local cache remains valid and can coexist with a
standard archive.

The Rust crate mirrors the distribution boundary with `profile-local` and
`profile-standard` Cargo features. The latter is the source-build default and
extends the former. OpenDAL facade defaults remain disabled: retry, timeout,
and concurrency support are compiled explicitly for the public immutable layer
constructors, but no layer is installed implicitly; logging and unrelated
layers remain absent. Both
`http-transport-reqwest` and its rustls backend are explicit because OpenDAL
0.58.1 gates the HTTP installation performed by `install_default()` on the
umbrella feature. Standard-profile runtime initialization calls
`opendal::install_default()`; calling only `init_default_registry()` would not
install the S3 HTTP transport.

## Supported host matrix

The immutable `local` v0.1 release remains available on its original two
hosts. The `v0.2.0` release expands `standard` to three target-native builders:

| Host | Release target | Compatibility floor | Profile |
| --- | --- | --- | --- |
| Apple silicon macOS | `aarch64-apple-darwin` | macOS 11.0 | `local`, `standard` |
| arm64 Linux | `aarch64-unknown-linux-gnu` | glibc 2.35 | `standard` |
| x86-64 Linux | `x86_64-unknown-linux-gnu` | glibc 2.35 | `local`, `standard` |

Every `standard` archive is built and linked on a GitHub-hosted runner with
the same architecture; no cross-compiled archive is presented as target-native
validation. These artifact and host constraints apply only when the
cross-target module is compiled for native; explicit JavaScript builds use the
embedded browser runtime and do not resolve this table. Unsupported native
hosts fail before downloading with a message listing the selected profile's
matrix.
On Linux arm64, Rust reports the unwind dependency as `-lgcc_s`, while the
MoonBit native linker cannot discover the unversioned development symlink.
The resolver therefore replaces only that flag with a verified, versioned
`libgcc_s.so.1` already supplied by the glibc host. This requires neither a C
compiler nor a development package and fails before linking when the runtime
is absent.
The current MoonBit CLI installer does not provide an Intel macOS toolchain,
so a Rust-only build on that host is not sufficient evidence for publishing a
MoonBit artifact. Intel macOS, Windows, and musl require separately built and
tested toolchain paths; the installer never guesses that one target is
compatible with another.

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
- the profile-level Cargo feature and required runtime initializer when the
  profile declares them;
- the exact Rust target and its compatibility floor;
- the static-library relative path, byte size, and SHA-256;
- the exact system libraries reported by
  `cargo rustc -- --print native-static-libs` on that target.

The release workflow produces archives with normalized ordering, owner,
permissions, and timestamps. It builds each target on that target rather than
assuming that one host's native-link report applies to another host.

## Trust and cache model

The selected profile table pins each archive URL, archive size and SHA-256,
extracted library size and SHA-256, manifest identity, compatibility floor,
and system link flags. A checksum downloaded beside a mutable archive is
useful for humans but is not the installer's trust root.

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
system libraries to the link configuration for `Eric-Song-Nop/opendal`.

## Maintainer path

Repository development still builds the Rust archive from source. Ordinary
source builds use `profile-standard`; an explicit local artifact rebuild uses
`make native NATIVE_SERVICE_PROFILE=local`, and its native contract suite uses
`make rust-test NATIVE_SERVICE_PROFILE=local`. The complete Phase 5 Moon test
suite runs against `profile-standard` because it intentionally exercises
standard-only S3 and operational-layer symbols. The Makefile selects the
requested Cargo feature explicitly and passes both the exact locally built
archive and its source profile to the configuration script. The script keeps
the committed artifact selection and pins untouched while overlaying the
maintainer-built archive for that invocation and adding any host frameworks
declared by the source profile. Moon tests therefore exercise uncommitted Rust
changes with matching link requirements instead of silently using a released
binary. These overrides are not needed or documented in the consumer
quickstart.

Source builds are also the fallback for maintainers adding a target or
service profile. They are not an automatic fallback for consumers: silently
invoking Cargo would violate the ordinary-package contract and make builds
depend on an unpinned host toolchain.

## Release and acceptance gates

Native artifact CI has two modes. Pull requests and manual dispatches use
**candidate mode**: the packager derives a temporary profile-specific table
from the archive it just built, the clean-consumer harness overlays that table
and its internal selection into a staged `moon package`, and the resolver fills
the cache from the local candidate bytes. Candidate records use the
non-resolving `candidate.invalid` origin and are never release trust roots.
Version tags use **release mode** and require every built field and digest to
match the committed profile table; candidate URLs are rejected.

A release is eligible only when all of the following pass:

1. Rust, C ABI, MoonBit, debug/release, sanitizer, and public-interface tests.
2. Reproducible artifact generation on every advertised host.
3. Resolver tests for target selection, integrity rejection, cache recovery,
   concurrency, and offline hot-cache use.
4. A consumer assembled from the `moon package` archive that has no repository
   checkout, Cargo/Rust on `PATH`, `LIBRARY_PATH`, or consumer linker flags.
5. That consumer runs every service workflow promised by the selected profile
   with ordinary `moon test --target native` commands on every advertised host.
6. Moon module, Rust crate and lockfile, and registry-consumer dependency
   versions are identical.

The release workflow publishes one selected-profile archive per supported host
under the package version tag, then publishes that exact source tree to
mooncakes.io and executes a fresh registry consumer. Adding a target means
building it in CI, recording
its own compatibility floor and native link flags, extending the pinned
artifact table, and adding the same clean-consumer gate for that host.
