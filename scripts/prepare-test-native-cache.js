'use strict';

const fs = require('node:fs');
const fsp = require('node:fs/promises');
const path = require('node:path');

async function main() {
  const [moduleRootArgument, archiveArgument, moonHomeArgument] = process.argv.slice(2);
  if (!moduleRootArgument || !archiveArgument || !moonHomeArgument) {
    throw new Error(
      'usage: prepare-test-native-cache.js <module-root> <archive> <moon-home>',
    );
  }
  const moduleRoot = path.resolve(moduleRootArgument);
  const archive = path.resolve(archiveArgument);
  const moonHome = path.resolve(moonHomeArgument);
  const resolver = require(path.join(moduleRoot, 'build.js'));
  const artifacts = resolver.loadArtifacts(
    path.join(moduleRoot, 'native', 'artifacts.json'),
  );
  const archiveName = path.basename(archive);
  const artifact = Object.values(artifacts).find(
    (candidate) => candidate.archive_name === archiveName,
  );
  if (!artifact) {
    throw new Error(`the package does not pin ${archiveName}`);
  }
  await resolver.ensureArtifact(moonHome, artifact, {
    download: async (_url, destination) => {
      await fsp.copyFile(archive, destination, fs.constants.COPYFILE_EXCL);
    },
  });
}

main().catch((error) => {
  console.error(`prepare-test-native-cache: ${error.message}`);
  process.exitCode = 1;
});
