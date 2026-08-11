# Release Procedure

The native assets and Moon package are one release unit. Do not publish either
side manually or reuse an asset built from a different commit.

## One-time repository setup

1. Run `moon login` with the mooncakes.io owner of
   `Eric-Song-Nop/opendal`.
2. Store the exact contents of `~/.moon/credentials.json` as the GitHub
   Actions secret `MOONCAKES_CREDENTIALS_JSON`.
3. Keep the secret scoped to this repository and rotate it if a release log or
   runner ever exposes it.

The release job checks that the secret is present before it creates or changes
a GitHub release.

## Version release

1. Update the version in `moon.mod` and `native/rust/Cargo.toml`.
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
   generated artifact's clean packaged-consumer test.
7. Merge the complete stack, then create and push the exact `v<moon.mod
   version>` tag. For `0.1.0`, the tag is `v0.1.0`.

The tag workflow runs in release mode, rebuilds the selected profile on both
target hosts, rejects candidate URLs, and rejects any digest difference from
that profile's committed table. It then:

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
must independently identify the standard service profile.

If a step fails after the GitHub release is created, rerunning the workflow
replaces its assets with the same verified bytes. Never change an existing
release asset without also changing its pinned digest and artifact revision.

`make version-contract` keeps `moon.mod`, the native Rust crate and lockfile,
and the versioned dependency in `integration/consumer/moon.mod` aligned. The
tag job additionally renders its verified tag version into a temporary copy of
that consumer, so the final registry acceptance job cannot silently resolve an
older release even when the repository fixture is stale.
