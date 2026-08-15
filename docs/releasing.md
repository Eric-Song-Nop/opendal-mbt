# Release Procedure

The native assets, checked-in embedded Browser runtime, and Moon package are
one release unit. Do not publish one side manually or reuse an asset or
generated Browser snapshot built from a different source state.

## One-time repository setup

1. Run `moon login` with the mooncakes.io owner of
   `Eric-Song-Nop/opendal`.
2. Store the exact contents of `~/.moon/credentials.json` as the GitHub
   Actions secret `MOONCAKES_CREDENTIALS_JSON`.
3. Keep the secret scoped to this repository and rotate it if a release log or
   runner ever exposes it.

The release job checks that the secret is present before it creates or changes
a GitHub release.

The tag job deliberately runs `moon publish` without `--frozen`. Publishing
validates the archive from a fresh extracted directory whose dependency cache
starts empty; it must be allowed to install the exact dependency version pinned
in `moon.mod`. The repository itself is still resolved before publication, and
the clean registry-consumer step verifies the published version afterward.

## Version release

1. Update the version in `moon.mod`, `native/rust/Cargo.toml`,
   `wasm/browser-bridge/Cargo.toml`, and
   `wasm/browser-runtime/contract.json`. Update the pinned integration consumer
   and refresh `Cargo.lock` through Cargo; do not edit lockfile package entries
   by hand.
2. Bump `artifact_revision` in the selected distribution profile when
   rebuilding artifacts for an unchanged binding version.
3. Confirm `native/artifact-selection.json` names exactly the profile shipped
   by the root facade. `local` remains the immutable `v0.1.x` profile;
   Phase 5 standard releases use `native/artifacts-standard.json`.
4. Run the native-artifact workflow on the candidate commit. Candidate mode
   emits one `*.candidate-artifacts.json` table per host and proves the staged
   Moon package against the newly built bytes. Merge each host record into the
   selected committed table and replace its `candidate.invalid` URL with the
   final release URL. The macOS standard record must retain the `Security` and
   `CoreFoundation` framework flags captured from `rustc`.
5. Rerun the workflow and require byte-identical archives before approving the
   pinned digests.
6. Run `make check`, debug/release `make test-profile`, `make asan`, and each
   generated native artifact's clean packaged-consumer test.
7. Merge the complete stack, then create and push the exact `v<moon.mod
   version>` tag. The `0.2.0` standard release uses tag `v0.2.0`; `v0.1.0`
   remains the historical local release.

`make test-profile` includes the shared native/browser application source and
the native delayed-S3 heartbeat probe. The probe must report that the MoonBit
scheduler ran while the OpenDAL future was pending, with async file-descriptor
leak checking enabled. The native artifact workflow repeats that proof on each
advertised release host before packaging its archive.

## Browser release checklist

Run this checklist on the exact candidate commit before creating the tag. The
Rust WebAssembly bridge is an internal implementation of MoonBit's JavaScript
target; this checklist does not claim support for MoonBit `wasm` or `wasm-gc`.

1. Install the pinned tools and confirm their versions:

   ```sh
   rustup target add wasm32-unknown-unknown
   cargo install --locked wasm-bindgen-cli --version 0.2.127
   wasm-bindgen --version
   ```

   The CLI must report exactly `wasm-bindgen 0.2.127`, matching `Cargo.lock`,
   the Makefile pin, the generated header, and `contract.json`.

2. Review `wasm/browser-runtime/contract.json` against the bridge and Promise
   runtime: binding version, packed Browser ABI `0x0001_0007`, required feature
   mask `0x0000_fffc`, services, hard limits, `ODE1`/`ODM1` snapshots, and
   local cancellation/busy error codes must agree.

3. Regenerate the committed embedded distribution after any version, bridge,
   runtime, generator, lockfile, or declared source-input change:

   ```sh
   make browser-embed-generate
   git diff -- src/browser/embedded_runtime.generated.mbt
   make browser-embed-check
   ```

   Review the generated wasm-bindgen/ABI/features line, source fingerprints,
   and payload change. Never edit the generated MoonBit file directly or make
   a stale check pass by removing a source from the fingerprint set.

4. Run all Browser implementation and target checks:

   ```sh
   make moon-deps
   make moon-browser-check
   make moon-browser-test
   make browser-rust-check
   make browser-rust-test
   make browser-js-canary RUST_PROFILE=release
   ```

   The canary must run the rebuilt module-form bridge in real Chrome; a Node
   process is only its launcher and local server.

5. Prove the embedded consumer and published package shape:

   ```sh
   make portable-async-example-browser
   make browser-demo
   make packaged-browser
   ```

   All three commands must complete a real Chrome round trip. The portable
   example runs the same application source as native, then requires its shared
   heartbeat to advance during a cross-origin delayed S3 read and reports both
   success markers. `packaged-browser` must run from a freshly packed module
   with Cargo, Rust, wasm-bindgen, npm, and common bundlers hidden, with no
   separately shipped `.wasm` or `.mjs` runtime asset.

6. Require the **Browser JS** workflow to be green on the same candidate
   commit. It repeats the Moon JS checks/tests, Rust checks/tests, Chrome
   canary, embedded snapshot check, shared non-blocking S3 proof, and
   packaged-browser proof.

See [Browser Runtime and Wasm ABI](design/browser-runtime.md) for the ownership,
task, cancellation, snapshot, and reproducibility contracts behind these
commands.

The native tag workflow runs in release mode and rebuilds the selected profile
on every advertised target host—three for the `v0.2.0` standard profile:
macOS arm64, Linux x86-64, and Linux arm64. It rejects candidate URLs and any
digest difference from the committed table, then:

1. uploads the archives, checksums, and manifests to the GitHub release;
2. publishes the source package to mooncakes.io;
3. renders the tag version into a fresh registry consumer and resolves that
   exact package version;
4. downloads the just-published native asset through `build.js`; and
5. runs the acceptance suite for every service promised by the selected
   profile.

Before upload, each artifact job links a strict C consumer with the manifest's
recorded system flags and asks the resulting library for `library_info`. The
reported binding version must match the package version; the artifact manifest
must independently identify the selected service profile (`standard` for
`v0.2.0`).

If a step fails after the GitHub release is created, rerunning the workflow
replaces its assets with the same verified bytes. Never change an existing
release asset without also changing its pinned digest and artifact revision.

`make version-contract` keeps `moon.mod`, the native Rust crate and lockfile,
and the versioned dependency in `integration/consumer/moon.mod` aligned. The
Browser checklist separately verifies the Browser crate, `contract.json`, and
regenerated embedded source. The tag job additionally renders its verified tag
version into a temporary copy of the integration consumer, so the final
registry acceptance job cannot silently resolve an older release even when the
repository fixture is stale.

The embedded Browser payload is committed source inside the Moon package; the
native tag workflow does not replace it with an independently built release
asset. The pre-tag Browser workflow and packaged-browser proof on the exact
candidate commit are therefore mandatory release evidence.
