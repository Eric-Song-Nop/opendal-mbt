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
2. Bump `artifact_revision` when rebuilding artifacts for an unchanged binding
   version or profile.
3. Run the native-artifact workflow on the candidate commit and update
   `native/artifacts.json` from its two outputs.
4. Rerun the workflow and require byte-identical archives before approving the
   pinned digests.
5. Run `make check`, debug/release `make test-profile`, `make asan`, and each
   generated artifact's clean packaged-consumer test.
6. Merge the complete stack, then create and push the exact `v<moon.mod
   version>` tag. For `0.1.0`, the tag is `v0.1.0`.

The tag workflow rebuilds both artifacts on their target hosts and rejects any
digest difference from `native/artifacts.json`. It then:

1. uploads the archives, checksums, and manifests to the GitHub release;
2. publishes the source package to mooncakes.io;
3. resolves that registry package in a fresh consumer;
4. downloads the just-published native asset through `build.js`; and
5. runs the memory and filesystem acceptance suite.

If a step fails after the GitHub release is created, rerunning the workflow
replaces its assets with the same verified bytes. Never change an existing
release asset without also changing its pinned digest and artifact revision.
