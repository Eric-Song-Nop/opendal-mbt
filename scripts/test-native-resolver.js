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
  loadArtifactSelection,
  loadArtifacts,
  loadDistributionProfile,
  loadSelectedArtifacts,
  makeBuildOutput,
  makeNoopBuildOutput,
  makeSourceBuildOutput,
  normalizeMoonTarget,
  parseMoonTarget,
  parseMaintainerLinkFlags,
  resolveMoonTarget,
  resolveSystemLinkFlags,
  resolveLocalOverride,
  selectArtifact,
  sha256File,
} = require('../build.js');

test('portable Moon targets use an empty link configuration', () => {
  assert.deepEqual(makeNoopBuildOutput(), { vars: {}, link_configs: [] });
  for (const target of ['js', 'wasm', 'wasm-gc']) {
    assert.equal(normalizeMoonTarget(target), target);
  }
  assert.equal(normalizeMoonTarget('unknown'), null);
});

test('Moon target resolver accepts protocol fields and explicit environment', () => {
  assert.equal(resolveMoonTarget({ target: 'js', env: {} }), 'js');
  assert.equal(resolveMoonTarget({ backend: 'wasm', env: {} }), 'wasm');
  assert.equal(resolveMoonTarget({ build_target: 'wasm-gc', env: {} }), 'wasm-gc');
  assert.equal(resolveMoonTarget({ paths: { target: 'native' }, env: {} }), 'native');
  assert.equal(
    resolveMoonTarget({ env: { OPENDAL_MBT_TARGET: 'all' } }),
    'all',
  );
});

test('Moon target resolver reads only the parent command target option', () => {
  assert.equal(parseMoonTarget('moon test --target js src/browser'), 'js');
  assert.equal(parseMoonTarget('moon build --target=wasm-gc'), 'wasm-gc');
  assert.equal(parseMoonTarget('moon test --target native src'), 'native');
  assert.equal(
    parseMoonTarget(
      '"C:\\Program Files\\MoonBit\\moon.exe" -C "C:\\work tree" test --target "js" src/browser',
    ),
    'js',
  );
  assert.equal(parseMoonTarget('moon run src/browser -- --target wasm'), null);
  assert.equal(
    resolveMoonTarget(
      { paths: { module_root: '/module', out_dir: 'TODO' }, env: {} },
      { parentCommandLine: 'moon test --target wasm src/browser' },
    ),
    'wasm',
  );
  assert.equal(
    resolveMoonTarget(
      { paths: { module_root: '/module', out_dir: 'TODO' }, env: {} },
      { parentCommandLine: 'moon test src' },
    ),
    null,
  );
});

test('Moon target resolver queries the parent process without a shell', () => {
  const calls = [];
  const spawn = (command, args, options) => {
    calls.push({ command, args, options });
    return { status: 0, stdout: 'moon test --target js src/browser\n' };
  };
  assert.equal(
    resolveMoonTarget(
      { paths: { module_root: '/module', out_dir: 'TODO' }, env: {} },
      { platform: 'darwin', spawnSync: spawn },
    ),
    'js',
  );
  assert.equal(calls[0].command, 'ps');
  assert.deepEqual(calls[0].options.stdio, ['ignore', 'pipe', 'ignore']);

  calls.length = 0;
  assert.equal(
    resolveMoonTarget(
      { paths: { module_root: 'C:\\module', out_dir: 'TODO' }, env: {} },
      { platform: 'win32', spawnSync: spawn },
    ),
    'js',
  );
  assert.equal(calls[0].command, 'powershell.exe');
  assert.equal(calls[0].args.includes('-NonInteractive'), true);
  assert.equal(calls[0].options.stdio[0], 'ignore');

  assert.equal(
    resolveMoonTarget(
      { paths: { module_root: 'C:\\module', out_dir: 'TODO' }, env: {} },
      {
        platform: 'win32',
        spawnSync: () => ({ status: 1, stdout: '', error: new Error('missing') }),
      },
    ),
    null,
  );
});

test('portable target prebuild bypasses native artifact resolution', () => {
  const result = spawnSync(process.execPath, [path.join(__dirname, '..', 'build.js')], {
    encoding: 'utf8',
    input: JSON.stringify({
      target: 'js',
      env: { OPENDAL_MBT_NATIVE_LIB: '/definitely/missing/native-library.a' },
    }),
  });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), { vars: {}, link_configs: [] });
  assert.equal(result.stderr, '');
});

test('native and unspecified targets preserve native artifact resolution', () => {
  for (const target of ['native', undefined]) {
    const input = {
      env: { OPENDAL_MBT_NATIVE_LIB: '/definitely/missing/native-library.a' },
    };
    if (target) {
      input.target = target;
    }
    const result = spawnSync(
      process.execPath,
      [path.join(__dirname, '..', 'build.js')],
      { encoding: 'utf8', input: JSON.stringify(input) },
    );
    assert.equal(result.status, 1);
    assert.match(result.stderr, /OPENDAL_MBT_NATIVE_LIB.*unavailable/);
  }
});

async function fixture(serviceProfile = 'local') {
  const root = await fsp.mkdtemp(path.join(os.tmpdir(), 'opendal-resolver-test-'));
  const source = path.join(root, 'source');
  await fsp.mkdir(path.join(source, 'lib'), { recursive: true });
  const library = path.join(source, 'lib', 'libopendal_mbt_native.a');
  await fsp.writeFile(library, Buffer.from('!<arch>\nresolver fixture'));
  await fsp.writeFile(path.join(source, 'LICENSE'), 'fixture license\n');
  const libraryStat = await fsp.stat(library);
  const librarySha256 = await sha256File(library);
  const standard = serviceProfile === 'standard';
  const artifact = {
    artifact: `opendal-mbt-0.1.0-r1-${serviceProfile}-aarch64-apple-darwin`,
    artifact_revision: 'r1',
    binding_version: '0.1.0',
    abi_version: { major: 1, minor: 0, patch: 0 },
    opendal_version: '0.58.1',
    rust_version: '1.91',
    service_profile: serviceProfile,
    services: standard ? ['memory', 'fs', 's3'] : ['memory', 'fs'],
    rust_features: standard
      ? [
          'blocking',
          'services-fs',
          'services-s3',
          'http-transport-reqwest',
          'http-transport-reqwest-rustls',
          'executors-tokio',
          'layers-retry',
          'layers-timeout',
          'layers-concurrent-limit',
        ]
      : ['blocking', 'services-fs'],
    rust_target: 'aarch64-apple-darwin',
    host_key: 'darwin-arm64',
    minimum_macos_version: '11.0',
    static_library: 'lib/libopendal_mbt_native.a',
    static_library_size: libraryStat.size,
    static_library_sha256: librarySha256,
    system_link_flags: ['-liconv', '-lSystem', '-lc', '-lm'],
    url: 'https://example.invalid/opendal.tar.gz',
  };
  if (standard) {
    artifact.cargo_features = ['profile-standard'];
    artifact.runtime_initialization = 'install_default';
  }
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

test('Linux arm64 uses the installed versioned GCC unwind runtime', async () => {
  const root = await fsp.mkdtemp(path.join(os.tmpdir(), 'opendal-gcc-runtime-test-'));
  try {
    const runtime = path.join(root, 'libgcc_s.so.1');
    await fsp.writeFile(runtime, 'runtime fixture');
    const resolvedRuntime = fs.realpathSync(runtime);
    const artifact = {
      host_key: 'linux-arm64',
      system_link_flags: ['-lgcc_s', '-lpthread', '-lc'],
    };
    const flags = resolveSystemLinkFlags(artifact, {
      gccRuntimeCandidates: [runtime],
    });
    assert.deepEqual(flags, [`'${resolvedRuntime}'`, '-lpthread', '-lc']);
    assert.equal(
      makeBuildOutput('/cache/libopendal.a', artifact, {
        gccRuntimeCandidates: [runtime],
      }).link_configs[0].link_flags,
      `'/cache/libopendal.a' '${resolvedRuntime}' -lpthread -lc`,
    );
  } finally {
    await cleanup(root);
  }
});

test('Linux arm64 fails early when the GCC unwind runtime is absent', () => {
  assert.throws(
    () =>
      resolveSystemLinkFlags(
        { host_key: 'linux-arm64', system_link_flags: ['-lgcc_s', '-lc'] },
        { gccRuntimeCandidates: ['/definitely/missing/libgcc_s.so.1'] },
      ),
    /No versioned libgcc_s runtime/,
  );
});

test('immutable local artifact table preserves the v0.1 host matrix', () => {
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

test('standard artifact table pins the complete v0.2 host matrix', () => {
  const artifacts = loadArtifacts(
    path.join(__dirname, '..', 'native', 'artifacts-standard.json'),
  );
  assert.deepEqual(Object.keys(artifacts).sort(), [
    'darwin-arm64',
    'linux-arm64',
    'linux-x64',
  ]);
  for (const [hostKey, artifact] of Object.entries(artifacts)) {
    assert.equal(artifact.host_key, hostKey);
    assert.equal(artifact.binding_version, '0.2.0');
    assert.equal(artifact.artifact_revision, 'r2');
    assert.equal(artifact.service_profile, 'standard');
    assert.equal(
      artifact.artifact,
      `opendal-mbt-0.2.0-r2-standard-${artifact.rust_target}`,
    );
    assert.equal(artifact.archive_name, `${artifact.artifact}.tar.gz`);
    assert.deepEqual(artifact.services, ['memory', 'fs', 's3']);
    assert.match(artifact.archive_sha256, /^[0-9a-f]{64}$/);
    assert.match(artifact.static_library_sha256, /^[0-9a-f]{64}$/);
    assert.equal(
      artifact.url,
      `https://github.com/Eric-Song-Nop/opendal-mbt/releases/download/v0.2.0/${artifact.archive_name}`,
    );
  }
});

test('published package selects one profile without consulting the environment', async () => {
  const root = await fsp.mkdtemp(path.join(os.tmpdir(), 'opendal-selection-test-'));
  try {
    const selection = path.join(root, 'artifact-selection.json');
    await fsp.writeFile(
      selection,
      JSON.stringify({
        schema_version: 1,
        service_profile: 'standard',
        artifact_table: 'artifacts-standard.json',
      }),
    );
    await fsp.writeFile(
      path.join(root, 'artifacts-standard.json'),
      JSON.stringify({
        schema_version: 1,
        service_profile: 'standard',
        artifacts: {
          'darwin-arm64': {
            artifact: 'standard-darwin',
            service_profile: 'standard',
          },
        },
      }),
    );
    process.env.OPENDAL_MBT_PROFILE = 'local';
    const selected = loadSelectedArtifacts(selection);
    assert.equal(selected.serviceProfile, 'standard');
    assert.equal(selected.artifacts['darwin-arm64'].artifact, 'standard-darwin');
  } finally {
    delete process.env.OPENDAL_MBT_PROFILE;
    await cleanup(root);
  }
});

test('artifact selection rejects path traversal', async () => {
  const root = await fsp.mkdtemp(path.join(os.tmpdir(), 'opendal-selection-test-'));
  try {
    const selection = path.join(root, 'artifact-selection.json');
    await fsp.writeFile(
      selection,
      JSON.stringify({
        schema_version: 1,
        service_profile: 'standard',
        artifact_table: '../artifacts.json',
      }),
    );
    assert.throws(() => loadArtifactSelection(selection), /Unsupported native artifact selection/);
  } finally {
    await cleanup(root);
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

test('local and standard artifacts use distinct cache roots', async () => {
  const local = await fixture('local');
  const standard = await fixture('standard');
  try {
    const moonHome = path.join(local.root, 'moon-home');
    const localPaths = cachePaths(moonHome, local.artifact);
    const standardPaths = cachePaths(moonHome, standard.artifact);
    assert.notEqual(localPaths.versionRoot, standardPaths.versionRoot);
    assert.match(localPaths.versionRoot, /\/local\//);
    assert.match(standardPaths.versionRoot, /\/standard\//);
  } finally {
    await cleanup(local.root);
    await cleanup(standard.root);
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
        package: 'Eric-Song-Nop/opendal',
        link_flags: "'/tmp/with space/lib'\"'\"'a.a' -lm -ldl",
      },
    ],
  });
});

test('unpinned Linux arm64 override resolves the versioned GCC runtime', async () => {
  const root = await fsp.mkdtemp(path.join(os.tmpdir(), 'opendal-override-test-'));
  try {
    const library = path.join(root, 'libopendal_mbt_native.a');
    const runtime = path.join(root, 'libgcc_s.so.1');
    await fsp.writeFile(library, '!<arch>\nmaintainer fixture');
    await fsp.writeFile(runtime, 'runtime fixture');
    const resolvedRuntime = fs.realpathSync(runtime);
    const output = await resolveLocalOverride(
      {
        env: {
          OPENDAL_MBT_NATIVE_LIB: library,
          OPENDAL_MBT_NATIVE_LIBS: '-lgcc_s -lpthread -lc',
        },
      },
      undefined,
      {
        hostKey: 'linux-arm64',
        gccRuntimeCandidates: [runtime],
      },
    );
    assert.equal(
      output.link_configs[0].link_flags,
      `'${library}' '${resolvedRuntime}' -lpthread -lc`,
    );
  } finally {
    await cleanup(root);
  }
});

test('standard source builds add only profile-required frameworks', () => {
  const profile = loadDistributionProfile('standard');
  const artifact = {
    rust_target: 'aarch64-apple-darwin',
    host_key: 'darwin-arm64',
    system_link_flags: ['-liconv', '-lSystem', '-lc', '-lm'],
  };
  const output = makeSourceBuildOutput('/source/libopendal.a', artifact, profile);
  assert.equal(
    output.link_configs[0].link_flags,
    "'/source/libopendal.a' -liconv -lSystem -lc -lm " +
      '-framework Security -framework CoreFoundation',
  );
  assert.deepEqual(artifact.system_link_flags, ['-liconv', '-lSystem', '-lc', '-lm']);
});

test('standard source builds do not duplicate artifact framework flags', () => {
  const profile = loadDistributionProfile('standard');
  const output = makeSourceBuildOutput(
    '/source/libopendal.a',
    {
      rust_target: 'aarch64-apple-darwin',
      host_key: 'darwin-arm64',
      system_link_flags: [
        '-framework',
        'Security',
        '-framework',
        'CoreFoundation',
        '-lc',
      ],
    },
    profile,
  );
  assert.equal(
    output.link_configs[0].link_flags,
    "'/source/libopendal.a' -framework Security -framework CoreFoundation -lc",
  );
});

test('explicit local source builds do not claim standard frameworks', () => {
  const profile = loadDistributionProfile('local');
  const output = makeSourceBuildOutput(
    '/source/libopendal.a',
    {
      rust_target: 'aarch64-apple-darwin',
      host_key: 'darwin-arm64',
      system_link_flags: ['-liconv', '-lSystem', '-lc', '-lm'],
    },
    profile,
  );
  assert.equal(
    output.link_configs[0].link_flags,
    "'/source/libopendal.a' -liconv -lSystem -lc -lm",
  );
});

test('maintainer link flags accept rustc native-static-libs syntax', () => {
  assert.deepEqual(
    parseMaintainerLinkFlags('-lc -lm -lrt -lpthread -framework Security'),
    ['-lc', '-lm', '-lrt', '-lpthread', '-framework', 'Security'],
  );
  assert.throws(
    () => parseMaintainerLinkFlags('-lc $(touch /tmp/not-allowed)'),
    /unsafe token/,
  );
});

test('maintainer override supports a host without a pinned artifact', async () => {
  const root = await fsp.mkdtemp(path.join(os.tmpdir(), 'opendal-override-test-'));
  try {
    const library = path.join(root, 'libopendal_mbt_native.a');
    await fsp.writeFile(library, '!<arch>\nmaintainer fixture');
    const output = await resolveLocalOverride(
      {
        env: {
          OPENDAL_MBT_NATIVE_LIB: library,
          OPENDAL_MBT_NATIVE_LIBS: '-lc -lm -lpthread',
        },
      },
      undefined,
    );
    assert.equal(
      output.link_configs[0].link_flags,
      `'${library}' -lc -lm -lpthread`,
    );
  } finally {
    await cleanup(root);
  }
});

test('unpinned maintainer override requires explicit native link flags', async () => {
  const root = await fsp.mkdtemp(path.join(os.tmpdir(), 'opendal-override-test-'));
  try {
    const library = path.join(root, 'libopendal_mbt_native.a');
    await fsp.writeFile(library, '!<arch>\nmaintainer fixture');
    await assert.rejects(
      resolveLocalOverride(
        { env: { OPENDAL_MBT_NATIVE_LIB: library } },
        undefined,
      ),
      /OPENDAL_MBT_NATIVE_LIBS is required/,
    );
  } finally {
    await cleanup(root);
  }
});
