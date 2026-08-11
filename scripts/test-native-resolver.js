'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const test = require('node:test');

const {
  cachePaths,
  ensureArtifact,
  loadArtifacts,
  makeBuildOutput,
  selectArtifact,
  sha256File,
} = require('../build.js');

async function fixture() {
  const root = await fsp.mkdtemp(path.join(os.tmpdir(), 'opendal-resolver-test-'));
  const source = path.join(root, 'source');
  await fsp.mkdir(path.join(source, 'lib'), { recursive: true });
  const library = path.join(source, 'lib', 'libopendal_mbt_native.a');
  await fsp.writeFile(library, Buffer.from('!<arch>\nresolver fixture'));
  await fsp.writeFile(path.join(source, 'LICENSE'), 'fixture license\n');
  const libraryStat = await fsp.stat(library);
  const librarySha256 = await sha256File(library);
  const artifact = {
    artifact: 'opendal-mbt-0.1.0-r1-local-aarch64-apple-darwin',
    artifact_revision: 'r1',
    binding_version: '0.1.0',
    abi_version: { major: 1, minor: 0, patch: 0 },
    opendal_version: '0.58.1',
    rust_version: '1.91',
    service_profile: 'local',
    services: ['memory', 'fs'],
    rust_features: ['blocking', 'services-fs'],
    rust_target: 'aarch64-apple-darwin',
    host_key: 'darwin-arm64',
    minimum_macos_version: '11.0',
    static_library: 'lib/libopendal_mbt_native.a',
    static_library_size: libraryStat.size,
    static_library_sha256: librarySha256,
    system_link_flags: ['-liconv', '-lSystem', '-lc', '-lm'],
    url: 'https://example.invalid/opendal.tar.gz',
  };
  await fsp.writeFile(
    path.join(source, 'manifest.json'),
    `${JSON.stringify({ schema_version: 1, ...artifact }, (key, value) => {
      if (['url', 'archive_size', 'archive_sha256'].includes(key)) {
        return undefined;
      }
      return value;
    }, 2)}\n`,
  );
  const archive = path.join(root, 'artifact.tar.gz');
  const packed = spawnSync(
    'tar',
    ['-czf', archive, '-C', source, 'LICENSE', 'lib', 'manifest.json'],
    { encoding: 'utf8' },
  );
  assert.equal(packed.status, 0, packed.stderr);
  artifact.archive_size = (await fsp.stat(archive)).size;
  artifact.archive_sha256 = await sha256File(archive);
  return { root, archive, artifact };
}

async function cleanup(root) {
  await fsp.rm(root, { recursive: true, force: true });
}

test('selectArtifact requires an exact host match', () => {
  const darwin = { artifact: 'darwin' };
  const artifacts = { 'darwin-arm64': darwin };
  assert.equal(selectArtifact(artifacts, 'darwin', 'arm64'), darwin);
  assert.throws(
    () => selectArtifact(artifacts, 'linux', 'x64'),
    /supported hosts: darwin-arm64/,
  );
});

test('published artifact table covers the initial host matrix', () => {
  const artifacts = loadArtifacts();
  assert.deepEqual(Object.keys(artifacts).sort(), ['darwin-arm64', 'linux-x64']);
  for (const [hostKey, artifact] of Object.entries(artifacts)) {
    assert.equal(artifact.host_key, hostKey);
    assert.equal(artifact.binding_version, '0.1.0');
    assert.equal(artifact.service_profile, 'local');
    assert.deepEqual(artifact.services, ['memory', 'fs']);
    assert.match(artifact.archive_sha256, /^[0-9a-f]{64}$/);
    assert.match(artifact.static_library_sha256, /^[0-9a-f]{64}$/);
    assert.equal(artifact.url.startsWith('https://github.com/'), true);
  }
});

test('cold install becomes a verified offline hot-cache hit', async () => {
  const value = await fixture();
  try {
    const moonHome = path.join(value.root, 'moon-home');
    let downloads = 0;
    const installed = await ensureArtifact(moonHome, value.artifact, {
      download: async (_url, destination) => {
        downloads += 1;
        await fsp.copyFile(value.archive, destination, fs.constants.COPYFILE_EXCL);
      },
    });
    assert.equal(downloads, 1);
    assert.equal(await sha256File(installed.staticLibrary), value.artifact.static_library_sha256);

    const offline = await ensureArtifact(moonHome, value.artifact, {
      download: async () => {
        throw new Error('network must not be used for a hot cache');
      },
    });
    assert.equal(offline.staticLibrary, installed.staticLibrary);
    assert.equal(downloads, 1);
  } finally {
    await cleanup(value.root);
  }
});

test('corrupt cache is quarantined and reinstalled', async () => {
  const value = await fixture();
  try {
    const moonHome = path.join(value.root, 'moon-home');
    let downloads = 0;
    const dependencies = {
      download: async (_url, destination) => {
        downloads += 1;
        await fsp.copyFile(value.archive, destination, fs.constants.COPYFILE_EXCL);
      },
    };
    const installed = await ensureArtifact(moonHome, value.artifact, dependencies);
    await fsp.appendFile(installed.staticLibrary, 'corruption');
    const repaired = await ensureArtifact(moonHome, value.artifact, dependencies);
    assert.equal(downloads, 2);
    assert.equal(await sha256File(repaired.staticLibrary), value.artifact.static_library_sha256);
    const { versionRoot } = cachePaths(moonHome, value.artifact);
    const entries = await fsp.readdir(versionRoot);
    assert.equal(entries.some((entry) => entry.includes('.corrupt-')), true);
  } finally {
    await cleanup(value.root);
  }
});

test('concurrent installers share one atomic cache fill', async () => {
  const value = await fixture();
  try {
    const moonHome = path.join(value.root, 'concurrent-moon-home');
    let downloads = 0;
    const dependencies = {
      download: async (_url, destination) => {
        downloads += 1;
        await new Promise((resolve) => setTimeout(resolve, 150));
        await fsp.copyFile(value.archive, destination, fs.constants.COPYFILE_EXCL);
      },
    };
    const [first, second] = await Promise.all([
      ensureArtifact(moonHome, value.artifact, dependencies),
      ensureArtifact(moonHome, value.artifact, dependencies),
    ]);
    assert.equal(downloads, 1);
    assert.equal(first.staticLibrary, second.staticLibrary);
  } finally {
    await cleanup(value.root);
  }
});

test('link configuration uses the exact archive path and target flags', () => {
  const output = makeBuildOutput("/tmp/with space/lib'a.a", {
    system_link_flags: ['-lm', '-ldl'],
  });
  assert.deepEqual(output, {
    vars: {},
    link_configs: [
      {
        package: 'eric-song-nop/opendal',
        link_flags: "'/tmp/with space/lib'\"'\"'a.a' -lm -ldl",
      },
    ],
  });
});
