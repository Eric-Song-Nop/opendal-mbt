'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const https = require('node:https');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { pipeline } = require('node:stream/promises');

const CACHE_SCHEMA_VERSION = 1;
const DOWNLOAD_ATTEMPTS = 3;
const DOWNLOAD_IDLE_TIMEOUT_MS = 30_000;
const LOCK_WAIT_TIMEOUT_MS = 120_000;
const STALE_LOCK_AGE_MS = 15 * 60_000;
const EXPECTED_ARCHIVE_ENTRIES = Object.freeze([
  'LICENSE',
  'lib',
  'lib/libopendal_mbt_native.a',
  'manifest.json',
]);

class CacheValidationError extends Error {
  constructor(message) {
    super(message);
    this.name = 'CacheValidationError';
  }
}

function report(message) {
  console.error(`[opendal.mbt] ${message}`);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function pathExists(filename) {
  try {
    await fsp.access(filename);
    return true;
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return false;
    }
    throw error;
  }
}

async function readBuildInput() {
  let source = '';
  process.stdin.setEncoding('utf8');
  for await (const chunk of process.stdin) {
    source += chunk;
  }
  if (source.trim() === '') {
    return { env: { ...process.env } };
  }

  let input;
  try {
    input = JSON.parse(source);
  } catch (error) {
    throw new Error(`Moon passed invalid build JSON: ${error.message}`);
  }
  if (!input || typeof input !== 'object' || Array.isArray(input)) {
    throw new Error('Moon build input must be a JSON object');
  }
  if (!input.env || typeof input.env !== 'object' || Array.isArray(input.env)) {
    input.env = { ...process.env };
  }
  return input;
}

function resolveMoonHome(input) {
  const configured = input.env.MOON_HOME;
  if (typeof configured === 'string' && configured.length > 0) {
    return path.resolve(configured);
  }
  const home = os.homedir();
  if (!home) {
    throw new Error('MOON_HOME is unset and the user home is unavailable');
  }
  return path.join(home, '.moon');
}

function loadArtifacts(filename = path.join(__dirname, 'native', 'artifacts.json')) {
  let source;
  try {
    source = fs.readFileSync(filename, 'utf8');
  } catch (error) {
    throw new Error(`Cannot read the pinned native artifact table: ${error.message}`);
  }
  let document;
  try {
    document = JSON.parse(source);
  } catch (error) {
    throw new Error(`The pinned native artifact table is invalid JSON: ${error.message}`);
  }
  if (
    !document ||
    document.schema_version !== 1 ||
    !document.artifacts ||
    typeof document.artifacts !== 'object' ||
    Array.isArray(document.artifacts)
  ) {
    throw new Error('Unsupported pinned native artifact table');
  }
  return document.artifacts;
}

function selectArtifact(artifacts, platform = process.platform, arch = process.arch) {
  const hostKey = `${platform}-${arch}`;
  const artifact = artifacts[hostKey];
  if (artifact) {
    return artifact;
  }
  throw new Error(
    `No opendal-mbt local artifact is available for ${hostKey}; ` +
      `supported hosts: ${Object.keys(artifacts).sort().join(', ')}`,
  );
}

function compareVersions(left, right) {
  const leftParts = left.split('.').map((part) => Number.parseInt(part, 10));
  const rightParts = right.split('.').map((part) => Number.parseInt(part, 10));
  const length = Math.max(leftParts.length, rightParts.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (leftParts[index] || 0) - (rightParts[index] || 0);
    if (difference !== 0) {
      return difference;
    }
  }
  return 0;
}

function validateHostCompatibility(artifact) {
  if (!artifact.minimum_glibc_version) {
    return;
  }
  const header = process.report?.getReport?.().header;
  const runtimeVersion = header && header.glibcVersionRuntime;
  if (!runtimeVersion) {
    throw new Error(
      `${artifact.artifact} requires glibc ${artifact.minimum_glibc_version} ` +
        'or newer; musl is not supported by this artifact',
    );
  }
  if (compareVersions(runtimeVersion, artifact.minimum_glibc_version) < 0) {
    throw new Error(
      `${artifact.artifact} requires glibc ${artifact.minimum_glibc_version} ` +
        `or newer, but this host reports ${runtimeVersion}`,
    );
  }
}

function cachePaths(moonHome, artifact) {
  const versionRoot = path.join(
    moonHome,
    'cache',
    'lib',
    'opendal-mbt',
    artifact.binding_version,
    artifact.service_profile,
    artifact.rust_target,
  );
  const installRoot = path.join(
    versionRoot,
    `sha256-${artifact.archive_sha256}`,
  );
  return {
    versionRoot,
    installRoot,
    lockPath: `${installRoot}.lock`,
  };
}

async function sha256File(filename) {
  const digest = crypto.createHash('sha256');
  const input = fs.createReadStream(filename);
  for await (const chunk of input) {
    digest.update(chunk);
  }
  return digest.digest('hex');
}

async function readJsonFile(filename, description) {
  let source;
  try {
    source = await fsp.readFile(filename, 'utf8');
  } catch (error) {
    throw new CacheValidationError(
      `Cannot read ${description} at ${filename}: ${error.message}`,
    );
  }
  try {
    return JSON.parse(source);
  } catch (error) {
    throw new CacheValidationError(`${description} is invalid JSON: ${error.message}`);
  }
}

function requireValid(condition, message) {
  if (!condition) {
    throw new CacheValidationError(message);
  }
}

function sameJson(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function safeInstallPath(installRoot, relativePath) {
  requireValid(
    typeof relativePath === 'string' && relativePath.length > 0,
    'Static library path is missing',
  );
  const resolvedRoot = path.resolve(installRoot);
  const resolved = path.resolve(resolvedRoot, relativePath);
  requireValid(
    resolved.startsWith(`${resolvedRoot}${path.sep}`),
    'Static library path escapes the artifact root',
  );
  return resolved;
}

async function requireRegularFile(filename, description) {
  let stat;
  try {
    stat = await fsp.lstat(filename);
  } catch (error) {
    throw new CacheValidationError(`${description} is unavailable: ${error.message}`);
  }
  requireValid(stat.isFile() && !stat.isSymbolicLink(), `${description} is not a regular file`);
  return stat;
}

async function validateInstalledArtifact(installRoot, artifact, requireMarker) {
  const manifestPath = path.join(installRoot, 'manifest.json');
  await requireRegularFile(manifestPath, 'Artifact manifest');
  await requireRegularFile(path.join(installRoot, 'LICENSE'), 'Artifact license');
  const manifest = await readJsonFile(manifestPath, 'artifact manifest');

  const exactFields = [
    'artifact',
    'artifact_revision',
    'binding_version',
    'opendal_version',
    'rust_version',
    'service_profile',
    'rust_target',
    'host_key',
    'static_library',
    'static_library_size',
    'static_library_sha256',
    'minimum_macos_version',
    'minimum_glibc_version',
  ];
  requireValid(manifest.schema_version === 1, 'Unsupported artifact manifest schema');
  for (const field of exactFields) {
    requireValid(
      manifest[field] === artifact[field],
      `Artifact manifest field ${field} does not match the pinned table`,
    );
  }
  for (const field of ['abi_version', 'services', 'rust_features', 'system_link_flags']) {
    requireValid(
      sameJson(manifest[field], artifact[field]),
      `Artifact manifest field ${field} does not match the pinned table`,
    );
  }

  const staticLibrary = safeInstallPath(installRoot, artifact.static_library);
  const libraryStat = await requireRegularFile(staticLibrary, 'Native static library');
  requireValid(
    libraryStat.size === artifact.static_library_size,
    `Native static library size mismatch: expected ${artifact.static_library_size}, ` +
      `got ${libraryStat.size}`,
  );
  const librarySha256 = await sha256File(staticLibrary);
  requireValid(
    librarySha256 === artifact.static_library_sha256,
    `Native static library SHA-256 mismatch: expected ` +
      `${artifact.static_library_sha256}, got ${librarySha256}`,
  );

  if (requireMarker) {
    const marker = await readJsonFile(
      path.join(installRoot, '.complete.json'),
      'cache completion marker',
    );
    requireValid(
      marker.cache_schema_version === CACHE_SCHEMA_VERSION,
      'Unsupported cache completion marker schema',
    );
    requireValid(marker.artifact === artifact.artifact, 'Cache marker artifact mismatch');
    requireValid(
      marker.archive_sha256 === artifact.archive_sha256,
      'Cache marker archive digest mismatch',
    );
    requireValid(
      marker.static_library_sha256 === artifact.static_library_sha256,
      'Cache marker library digest mismatch',
    );
  }
  return { staticLibrary };
}

async function tryInstalledArtifact(installRoot, artifact) {
  if (!(await pathExists(installRoot))) {
    return null;
  }
  try {
    return await validateInstalledArtifact(installRoot, artifact, true);
  } catch (error) {
    if (error instanceof CacheValidationError) {
      return null;
    }
    throw error;
  }
}

function downloadOnce(url, destination, expectedSize, redirects = 0) {
  return new Promise((resolve, reject) => {
    let parsed;
    try {
      parsed = new URL(url);
    } catch (error) {
      reject(new Error(`Invalid artifact URL ${url}: ${error.message}`));
      return;
    }
    if (parsed.protocol !== 'https:') {
      reject(new Error(`Refusing non-HTTPS artifact URL: ${url}`));
      return;
    }

    const request = https.get(
      parsed,
      {
        headers: {
          Accept: 'application/octet-stream',
          'User-Agent': 'opendal-mbt-build.js/1',
        },
      },
      (response) => {
        const status = response.statusCode || 0;
        if ([301, 302, 303, 307, 308].includes(status)) {
          response.resume();
          if (redirects >= 10 || !response.headers.location) {
            reject(new Error(`Cannot follow artifact redirect from ${url}`));
            return;
          }
          const redirected = new URL(response.headers.location, parsed).toString();
          downloadOnce(redirected, destination, expectedSize, redirects + 1)
            .then(resolve, reject);
          return;
        }
        if (status !== 200) {
          response.resume();
          reject(new Error(`Artifact download returned HTTP ${status} for ${url}`));
          return;
        }
        response.setTimeout(DOWNLOAD_IDLE_TIMEOUT_MS, () => {
          response.destroy(new Error('Artifact download stalled'));
        });
        const output = fs.createWriteStream(destination, { flags: 'wx', mode: 0o600 });
        pipeline(response, output)
          .then(async () => {
            const stat = await fsp.stat(destination);
            if (stat.size !== expectedSize) {
              throw new Error(
                `Artifact size mismatch: expected ${expectedSize}, got ${stat.size}`,
              );
            }
          })
          .then(resolve, reject);
      },
    );
    request.setTimeout(DOWNLOAD_IDLE_TIMEOUT_MS, () => {
      request.destroy(new Error('Artifact download connection timed out'));
    });
    request.on('error', reject);
  });
}

async function downloadArtifact(url, destination, expectedSize) {
  let lastError;
  for (let attempt = 1; attempt <= DOWNLOAD_ATTEMPTS; attempt += 1) {
    await fsp.rm(destination, { force: true });
    try {
      await downloadOnce(url, destination, expectedSize);
      return;
    } catch (error) {
      lastError = error;
      if (attempt < DOWNLOAD_ATTEMPTS) {
        report(`Download attempt ${attempt} failed: ${error.message}; retrying`);
        await delay(250 * attempt);
      }
    }
  }
  throw lastError;
}

function normalizeArchiveEntry(entry) {
  let normalized = entry.trim();
  while (normalized.startsWith('./')) {
    normalized = normalized.slice(2);
  }
  while (normalized.endsWith('/')) {
    normalized = normalized.slice(0, -1);
  }
  return normalized;
}

function extractArchive(archive, destination) {
  const listing = spawnSync('tar', ['-tzf', archive], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if (listing.error || listing.status !== 0) {
    throw new Error(
      `Cannot inspect native artifact: ${listing.error?.message || listing.stderr.trim()}`,
    );
  }
  const entries = listing.stdout
    .split('\n')
    .map(normalizeArchiveEntry)
    .filter((entry) => entry.length > 0)
    .sort();
  const expected = [...EXPECTED_ARCHIVE_ENTRIES].sort();
  if (!sameJson(entries, expected)) {
    throw new Error(`Native artifact has unexpected entries: ${entries.join(', ')}`);
  }
  for (const entry of entries) {
    if (path.isAbsolute(entry) || entry.split('/').includes('..')) {
      throw new Error(`Native artifact contains an unsafe path: ${entry}`);
    }
  }

  const extracted = spawnSync('tar', ['-xzf', archive, '-C', destination], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if (extracted.error || extracted.status !== 0) {
    throw new Error(
      `Cannot extract native artifact: ` +
        `${extracted.error?.message || extracted.stderr.trim()}`,
    );
  }
}

async function quarantineInvalidCache(installRoot) {
  if (!(await pathExists(installRoot))) {
    return;
  }
  const quarantine = `${installRoot}.corrupt-${Date.now()}-${process.pid}`;
  await fsp.rename(installRoot, quarantine);
  report(`Moved invalid cache entry to ${quarantine}`);
}

async function acquireInstallLock(lockPath, installRoot, artifact) {
  const deadline = Date.now() + LOCK_WAIT_TIMEOUT_MS;
  for (;;) {
    try {
      await fsp.mkdir(lockPath);
      return true;
    } catch (error) {
      if (!error || error.code !== 'EEXIST') {
        throw error;
      }
    }

    const ready = await tryInstalledArtifact(installRoot, artifact);
    if (ready) {
      return false;
    }
    try {
      const stat = await fsp.stat(lockPath);
      if (Date.now() - stat.mtimeMs > STALE_LOCK_AGE_MS) {
        await fsp.rmdir(lockPath);
        report(`Removed stale native artifact lock ${lockPath}`);
        continue;
      }
    } catch (error) {
      if (error && error.code === 'ENOENT') {
        continue;
      }
      if (error && (error.code === 'ENOTEMPTY' || error.code === 'EEXIST')) {
        // Another process still owns the lock.
      } else {
        throw error;
      }
    }
    if (Date.now() >= deadline) {
      throw new Error(`Timed out waiting for native artifact lock ${lockPath}`);
    }
    await delay(100);
  }
}

async function ensureArtifact(moonHome, artifact, dependencies = {}) {
  const download = dependencies.download || downloadArtifact;
  const extract = dependencies.extract || extractArchive;
  const { versionRoot, installRoot, lockPath } = cachePaths(moonHome, artifact);
  await fsp.mkdir(versionRoot, { recursive: true });

  const cached = await tryInstalledArtifact(installRoot, artifact);
  if (cached) {
    report(`Using cached ${artifact.artifact}`);
    return cached;
  }
  const ownsLock = await acquireInstallLock(lockPath, installRoot, artifact);
  if (!ownsLock) {
    return validateInstalledArtifact(installRoot, artifact, true);
  }

  let archivePath;
  let stagingRoot;
  try {
    const afterLock = await tryInstalledArtifact(installRoot, artifact);
    if (afterLock) {
      return afterLock;
    }
    await quarantineInvalidCache(installRoot);

    archivePath = path.join(
      versionRoot,
      `.download-${process.pid}-${crypto.randomUUID()}.tar.gz`,
    );
    report(`Downloading ${artifact.artifact}`);
    await download(artifact.url, archivePath, artifact.archive_size);
    const archiveStat = await requireRegularFile(archivePath, 'Downloaded artifact');
    requireValid(
      archiveStat.size === artifact.archive_size,
      `Downloaded artifact size mismatch: expected ${artifact.archive_size}, ` +
        `got ${archiveStat.size}`,
    );
    const archiveSha256 = await sha256File(archivePath);
    requireValid(
      archiveSha256 === artifact.archive_sha256,
      `Downloaded artifact SHA-256 mismatch: expected ${artifact.archive_sha256}, ` +
        `got ${archiveSha256}`,
    );

    stagingRoot = await fsp.mkdtemp(path.join(versionRoot, '.install-'));
    extract(archivePath, stagingRoot);
    await validateInstalledArtifact(stagingRoot, artifact, false);
    const marker = {
      cache_schema_version: CACHE_SCHEMA_VERSION,
      artifact: artifact.artifact,
      archive_sha256: artifact.archive_sha256,
      static_library_sha256: artifact.static_library_sha256,
    };
    await fsp.writeFile(
      path.join(stagingRoot, '.complete.json'),
      `${JSON.stringify(marker, null, 2)}\n`,
      { encoding: 'utf8', mode: 0o644 },
    );
    await fsp.rename(stagingRoot, installRoot);
    stagingRoot = undefined;
    report(`Installed ${artifact.artifact}`);
    return await validateInstalledArtifact(installRoot, artifact, true);
  } finally {
    if (archivePath) {
      await fsp.rm(archivePath, { force: true });
    }
    if (stagingRoot) {
      await fsp.rm(stagingRoot, { recursive: true, force: true });
    }
    await fsp.rmdir(lockPath).catch((error) => {
      if (!error || error.code !== 'ENOENT') {
        throw error;
      }
    });
  }
}

function shellQuote(value) {
  return `'${value.replaceAll("'", `'"'"'`)}'`;
}

function makeBuildOutput(staticLibrary, artifact) {
  return {
    vars: {},
    link_configs: [
      {
        package: 'eric-song-nop/opendal',
        link_flags: [shellQuote(staticLibrary), ...artifact.system_link_flags].join(' '),
      },
    ],
  };
}

async function resolveLocalOverride(input, artifact) {
  const configured = input.env.OPENDAL_MBT_NATIVE_LIB;
  if (typeof configured !== 'string' || configured.length === 0) {
    return null;
  }
  const staticLibrary = path.resolve(configured);
  const stat = await requireRegularFile(staticLibrary, 'OPENDAL_MBT_NATIVE_LIB');
  requireValid(stat.size > 0, 'OPENDAL_MBT_NATIVE_LIB is empty');
  report(`Using maintainer native library ${staticLibrary}`);
  return makeBuildOutput(staticLibrary, artifact);
}

function formatTopLevelError(error) {
  if (error && error.code === 'ENOENT') {
    return `${error.message}. Node.js and tar are required for native package installation.`;
  }
  if (error && error.code === 'EACCES') {
    return `Cannot write the opendal-mbt shared cache: ${error.message}`;
  }
  return error && error.message ? error.message : String(error);
}

async function main() {
  const nodeMajor = Number.parseInt(process.versions.node.split('.')[0], 10);
  if (nodeMajor < 18) {
    throw new Error(`Node.js 18 or newer is required; found ${process.versions.node}`);
  }
  const input = await readBuildInput();
  const artifact = selectArtifact(loadArtifacts());
  validateHostCompatibility(artifact);
  const localOutput = await resolveLocalOverride(input, artifact);
  if (localOutput) {
    process.stdout.write(`${JSON.stringify(localOutput)}\n`);
    return;
  }
  const installed = await ensureArtifact(resolveMoonHome(input), artifact);
  process.stdout.write(`${JSON.stringify(makeBuildOutput(installed.staticLibrary, artifact))}\n`);
}

module.exports = {
  CacheValidationError,
  cachePaths,
  compareVersions,
  ensureArtifact,
  extractArchive,
  loadArtifacts,
  makeBuildOutput,
  selectArtifact,
  sha256File,
  validateInstalledArtifact,
};

if (require.main === module) {
  main().catch((error) => {
    report(`Error: ${formatTopLevelError(error)}`);
    process.exitCode = 1;
  });
}
