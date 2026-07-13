import { createHash, randomUUID } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

function sha256File(filePath) {
  const hash = createHash('sha256');
  hash.update(fs.readFileSync(filePath));
  return hash.digest('hex');
}

function loadAndroidSherpaSource(sourceLockPath) {
  const source = JSON.parse(fs.readFileSync(sourceLockPath, 'utf8'));
  if (typeof source.version !== 'string' || source.version.length === 0) {
    throw new Error('Invalid sherpa-onnx Android source lock: version is required');
  }
  if (typeof source.url !== 'string' || !/^https:\/\//u.test(source.url)) {
    throw new Error('Invalid sherpa-onnx Android source lock: HTTPS URL is required');
  }
  if (typeof source.sha256 !== 'string' || !/^[a-f0-9]{64}$/u.test(source.sha256)) {
    throw new Error('Invalid sherpa-onnx Android source lock: SHA-256 must contain 64 lowercase hex characters');
  }
  if (!Array.isArray(source.abis) || source.abis.length === 0) {
    throw new Error('Invalid sherpa-onnx Android source lock: at least one ABI is required');
  }
  if (!Array.isArray(source.runtimeLibraries) || source.runtimeLibraries.length === 0) {
    throw new Error('Invalid sherpa-onnx Android source lock: runtime libraries are required');
  }
  return source;
}

function assertArchiveChecksum(archivePath, expectedSha256) {
  const actualSha256 = sha256File(archivePath);
  if (actualSha256 !== expectedSha256.toLowerCase()) {
    throw new Error(
      `sherpa-onnx Android archive SHA-256 mismatch: expected ${expectedSha256}, received ${actualSha256}`,
    );
  }
}

function assertChildPath(rootDir, childPath) {
  const relative = path.relative(path.resolve(rootDir), path.resolve(childPath));
  if (relative === '' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw new Error(`Refusing to modify a path outside the Android sherpa cache: ${childPath}`);
  }
}

function removeCachePath(cacheRoot, targetPath) {
  assertChildPath(cacheRoot, targetPath);
  fs.rmSync(targetPath, { recursive: true, force: true });
}

function validateRuntimeLayout(rootDir, source, selectedAbis) {
  for (const abi of selectedAbis) {
    if (!source.abis.includes(abi)) {
      throw new Error(`sherpa-onnx Android source does not support ABI ${abi}`);
    }

    const abiDir = path.join(rootDir, 'jniLibs', abi);
    if (!fs.existsSync(abiDir)) {
      throw new Error(`sherpa-onnx Android archive is missing Android ABI ${abi}`);
    }
    for (const library of source.runtimeLibraries) {
      if (!fs.existsSync(path.join(abiDir, library))) {
        throw new Error(`sherpa-onnx Android archive is missing ${library} for Android ABI ${abi}`);
      }
    }
  }
}

function extractArchive(archivePath, destinationDir) {
  const result = spawnSync('tar', ['-xjf', archivePath, '-C', destinationDir], {
    encoding: 'utf8',
  });
  if (result.error) {
    throw new Error(`Unable to extract sherpa-onnx Android archive: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const detail = result.stderr.trim() || result.stdout.trim() || `exit code ${result.status}`;
    throw new Error(`Unable to extract sherpa-onnx Android archive: ${detail}`);
  }
}

function readCompletionMarker(markerPath) {
  try {
    return JSON.parse(fs.readFileSync(markerPath, 'utf8'));
  } catch {
    return null;
  }
}

async function downloadVerifiedArchive(url, destinationPath, expectedSha256) {
  const temporaryPath = `${destinationPath}.tmp-${process.pid}-${randomUUID()}`;
  let lastError;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    fs.rmSync(temporaryPath, { force: true });
    try {
      const response = await fetch(url);
      if (!response.ok) {
        throw new Error(`${response.status} ${response.statusText}`.trim());
      }
      const archive = Buffer.from(await response.arrayBuffer());
      const contentLength = response.headers.get('content-length');
      if (contentLength !== null && archive.length !== Number(contentLength)) {
        throw new Error(`incomplete response: expected ${contentLength} bytes, received ${archive.length}`);
      }
      if (archive.length === 0) {
        throw new Error('empty response');
      }
      fs.writeFileSync(temporaryPath, archive);
      assertArchiveChecksum(temporaryPath, expectedSha256);
      try {
        fs.renameSync(temporaryPath, destinationPath);
      } catch (error) {
        if (!fs.existsSync(destinationPath)) {
          throw error;
        }
        assertArchiveChecksum(destinationPath, expectedSha256);
        fs.rmSync(temporaryPath, { force: true });
      }
      return destinationPath;
    } catch (error) {
      lastError = error;
      fs.rmSync(temporaryPath, { force: true });
    }
  }
  throw new Error(`Unable to download sherpa-onnx Android archive after 3 attempts: ${lastError.message}`);
}

function cachedRuntimeIsValid(runtimeRoot, source, selectedAbis) {
  const marker = readCompletionMarker(path.join(runtimeRoot, '.complete.json'));
  if (marker?.version !== source.version || marker?.sha256 !== source.sha256) {
    return false;
  }
  try {
    validateRuntimeLayout(runtimeRoot, source, selectedAbis);
    return true;
  } catch {
    return false;
  }
}

function findCachedRuntime(versionRoot, source, selectedAbis) {
  const entries = fs.readdirSync(versionRoot, { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isDirectory() || !entry.name.startsWith('runtime-')) {
      continue;
    }
    const runtimeRoot = path.join(versionRoot, entry.name);
    if (cachedRuntimeIsValid(runtimeRoot, source, selectedAbis)) {
      return runtimeRoot;
    }
  }
  return null;
}

async function prepareAndroidSherpaRuntime({
  source,
  cacheRoot,
  selectedAbis,
  archiveOverride,
}) {
  const versionRoot = path.join(path.resolve(cacheRoot), source.version);
  fs.mkdirSync(versionRoot, { recursive: true });
  let archivePath;
  if (archiveOverride) {
    archivePath = path.resolve(archiveOverride);
    if (!fs.existsSync(archivePath)) {
      throw new Error(`sherpa-onnx Android archive does not exist: ${archivePath}`);
    }
    assertArchiveChecksum(archivePath, source.sha256);
  }

  const cachedRuntimeRoot = findCachedRuntime(versionRoot, source, selectedAbis);
  if (cachedRuntimeRoot) {
    return { rootDir: cachedRuntimeRoot, source };
  }

  if (!archivePath) {
    archivePath = path.join(versionRoot, path.basename(new URL(source.url).pathname));
    let cachedArchiveValid = false;
    if (fs.existsSync(archivePath)) {
      try {
        assertArchiveChecksum(archivePath, source.sha256);
        cachedArchiveValid = true;
      } catch {
        fs.rmSync(archivePath, { force: true });
      }
    }
    if (!cachedArchiveValid) {
      await downloadVerifiedArchive(source.url, archivePath, source.sha256);
    }
  }

  const generationId = `${process.pid}-${Date.now()}-${randomUUID()}`;
  const extractionRoot = path.join(versionRoot, `runtime.tmp-${generationId}`);
  const runtimeRoot = path.join(versionRoot, `runtime-${generationId}`);
  fs.mkdirSync(extractionRoot, { recursive: true });

  try {
    extractArchive(archivePath, extractionRoot);
    validateRuntimeLayout(extractionRoot, source, selectedAbis);
    fs.writeFileSync(
      path.join(extractionRoot, '.complete.json'),
      `${JSON.stringify({
        version: source.version,
        sha256: source.sha256,
      }, null, 2)}\n`,
    );
    fs.renameSync(extractionRoot, runtimeRoot);
  } catch (error) {
    removeCachePath(versionRoot, extractionRoot);
    throw error;
  }

  return { rootDir: runtimeRoot, source };
}

function stageAndroidSherpaRuntime(prepared, abi, outDir) {
  validateRuntimeLayout(prepared.rootDir, prepared.source, [abi]);
  const sourceDir = path.join(prepared.rootDir, 'jniLibs', abi);
  const destinationDir = path.join(outDir, abi);
  fs.mkdirSync(destinationDir, { recursive: true });
  for (const library of prepared.source.runtimeLibraries) {
    fs.copyFileSync(path.join(sourceDir, library), path.join(destinationDir, library));
  }
}

export {
  assertArchiveChecksum,
  loadAndroidSherpaSource,
  prepareAndroidSherpaRuntime,
  sha256File,
  stageAndroidSherpaRuntime,
  validateRuntimeLayout,
};
