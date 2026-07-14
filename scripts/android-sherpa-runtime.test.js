import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { pathToFileURL } from 'node:url';
import { repoRoot } from './test-support/repo-root.js';
import { node } from './test-support/android-ndk-fixtures.js';

const androidSherpaRuntimePath = path.join(repoRoot, 'scripts', 'android-sherpa-runtime.js');

function createSherpaArchiveFixture({ abis = ['arm64-v8a'], omittedLibraries = [] } = {}) {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-android-sherpa-fixture-'));
  const archiveRoot = path.join(fixtureRoot, 'archive-root');
  const archivePath = path.join(fixtureRoot, 'sherpa-android.tar.bz2');
  const runtimeLibraries = ['libsherpa-onnx-c-api.so', 'libonnxruntime.so'];

  for (const abi of abis) {
    const abiDir = path.join(archiveRoot, 'jniLibs', abi);
    fs.mkdirSync(abiDir, { recursive: true });
    for (const library of runtimeLibraries) {
      if (!omittedLibraries.includes(library)) {
        fs.writeFileSync(path.join(abiDir, library), `${abi}:${library}`);
      }
    }
  }

  const tarResult = spawnSync('tar', ['-cjf', archivePath, '-C', archiveRoot, '.'], {
    encoding: 'utf8',
  });
  assert.equal(tarResult.status, 0, tarResult.stderr || tarResult.stdout);

  const sha256 = createHash('sha256').update(fs.readFileSync(archivePath)).digest('hex');
  return {
    archivePath,
    fixtureRoot,
    source: {
      version: 'test-1.0.0',
      url: 'https://example.invalid/sherpa-android.tar.bz2',
      sha256,
      abis: ['arm64-v8a', 'armeabi-v7a', 'x86', 'x86_64'],
      runtimeLibraries,
    },
  };
}

async function loadAndroidSherpaRuntime() {
  assert.equal(fs.existsSync(androidSherpaRuntimePath), true);
  return import(pathToFileURL(androidSherpaRuntimePath).href);
}

test('Android sherpa extraction selects the Windows system tar outside shell PATH', async () => {
  const { selectArchiveTarCommand } = await loadAndroidSherpaRuntime();
  const systemRoot = path.join('C:', 'Windows');
  const systemTar = path.join(systemRoot, 'System32', 'tar.exe');

  assert.equal(
    selectArchiveTarCommand({
      platform: 'win32',
      systemRoot,
      pathExists: (candidate) => candidate === systemTar,
    }),
    systemTar,
  );
  assert.equal(
    selectArchiveTarCommand({
      platform: 'linux',
      systemRoot: null,
      pathExists: () => false,
    }),
    'tar',
  );
});
function runSherpaPrepareProcess(source, cacheRoot) {
  const script = `
    const { prepareAndroidSherpaRuntime } = await import(process.env.SONA_TEST_SHERPA_MODULE_URL);
    const source = JSON.parse(Buffer.from(process.env.SONA_TEST_SHERPA_SOURCE, 'base64').toString('utf8'));
    await prepareAndroidSherpaRuntime({
      source,
      cacheRoot: process.env.SONA_TEST_SHERPA_CACHE,
      selectedAbis: ['arm64-v8a'],
    });
  `;
  const child = spawn(node, ['--input-type=module', '--eval', script], {
    cwd: repoRoot,
    env: {
      ...process.env,
      SONA_TEST_SHERPA_MODULE_URL: pathToFileURL(androidSherpaRuntimePath).href,
      SONA_TEST_SHERPA_SOURCE: Buffer.from(JSON.stringify(source)).toString('base64'),
      SONA_TEST_SHERPA_CACHE: cacheRoot,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let stdout = '';
  let stderr = '';
  child.stdout.setEncoding('utf8');
  child.stderr.setEncoding('utf8');
  child.stdout.on('data', (chunk) => { stdout += chunk; });
  child.stderr.on('data', (chunk) => { stderr += chunk; });
  return new Promise((resolve) => {
    child.on('close', (status) => resolve({ status, stdout, stderr }));
  });
}

test('Android sherpa runtime prepares and reuses a verified local archive', async () => {
  const { prepareAndroidSherpaRuntime } = await loadAndroidSherpaRuntime();
  const fixture = createSherpaArchiveFixture();
  const cacheRoot = path.join(fixture.fixtureRoot, 'cache');
  const prepared = await prepareAndroidSherpaRuntime({
    source: fixture.source,
    cacheRoot,
    selectedAbis: ['arm64-v8a'],
    archiveOverride: fixture.archivePath,
  });

  for (const library of fixture.source.runtimeLibraries) {
    assert.equal(fs.existsSync(path.join(prepared.rootDir, 'jniLibs', 'arm64-v8a', library)), true);
  }

  const sentinelPath = path.join(prepared.rootDir, 'cache-sentinel');
  fs.writeFileSync(sentinelPath, 'preserved');
  const reused = await prepareAndroidSherpaRuntime({
    source: fixture.source,
    cacheRoot,
    selectedAbis: ['arm64-v8a'],
    archiveOverride: fixture.archivePath,
  });
  assert.equal(reused.rootDir, prepared.rootDir);
  assert.equal(fs.readFileSync(sentinelPath, 'utf8'), 'preserved');
});

test('Android sherpa runtime rejects an archive with the wrong SHA-256', async () => {
  const { prepareAndroidSherpaRuntime } = await loadAndroidSherpaRuntime();
  const fixture = createSherpaArchiveFixture();

  await assert.rejects(
    prepareAndroidSherpaRuntime({
      source: { ...fixture.source, sha256: '0'.repeat(64) },
      cacheRoot: path.join(fixture.fixtureRoot, 'cache'),
      selectedAbis: ['arm64-v8a'],
      archiveOverride: fixture.archivePath,
    }),
    /sherpa-onnx Android archive SHA-256 mismatch/u,
  );
});

test('Android sherpa runtime rejects an archive missing a selected ABI', async () => {
  const { prepareAndroidSherpaRuntime } = await loadAndroidSherpaRuntime();
  const fixture = createSherpaArchiveFixture();

  await assert.rejects(
    prepareAndroidSherpaRuntime({
      source: fixture.source,
      cacheRoot: path.join(fixture.fixtureRoot, 'cache'),
      selectedAbis: ['x86_64'],
      archiveOverride: fixture.archivePath,
    }),
    /missing Android ABI x86_64/u,
  );
});

test('Android sherpa runtime rejects an archive missing a required library', async () => {
  const { prepareAndroidSherpaRuntime } = await loadAndroidSherpaRuntime();
  const fixture = createSherpaArchiveFixture({ omittedLibraries: ['libonnxruntime.so'] });

  await assert.rejects(
    prepareAndroidSherpaRuntime({
      source: fixture.source,
      cacheRoot: path.join(fixture.fixtureRoot, 'cache'),
      selectedAbis: ['arm64-v8a'],
      archiveOverride: fixture.archivePath,
    }),
    /missing libonnxruntime\.so for Android ABI arm64-v8a/u,
  );
});

test('Android sherpa runtime retries transient downloads before preparing the cache', async (t) => {
  const { prepareAndroidSherpaRuntime } = await loadAndroidSherpaRuntime();
  const fixture = createSherpaArchiveFixture();
  const archive = fs.readFileSync(fixture.archivePath);
  let attempts = 0;
  const server = http.createServer((request, response) => {
    attempts += 1;
    if (attempts < 3) {
      response.writeHead(503, { 'content-type': 'text/plain' });
      response.end('retry');
      return;
    }
    response.writeHead(200, {
      'content-length': archive.length,
      'content-type': 'application/octet-stream',
    });
    response.end(archive);
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve) => server.close(resolve)));
  const address = server.address();
  assert.notEqual(address, null);
  assert.equal(typeof address, 'object');

  const prepared = await prepareAndroidSherpaRuntime({
    source: {
      ...fixture.source,
      url: `http://127.0.0.1:${address.port}/sherpa-android.tar.bz2`,
    },
    cacheRoot: path.join(fixture.fixtureRoot, 'download-cache'),
    selectedAbis: ['arm64-v8a'],
  });

  assert.equal(attempts, 3);
  assert.equal(
    fs.existsSync(path.join(prepared.rootDir, 'jniLibs', 'arm64-v8a', 'libonnxruntime.so')),
    true,
  );
});

test('Android sherpa runtime atomically publishes concurrent preparations', async (t) => {
  const fixture = createSherpaArchiveFixture();
  const archive = fs.readFileSync(fixture.archivePath);
  let requests = 0;
  const server = http.createServer((request, response) => {
    requests += 1;
    setTimeout(() => {
      response.writeHead(200, {
        'content-length': archive.length,
        'content-type': 'application/octet-stream',
      });
      response.end(archive);
    }, 250);
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve) => server.close(resolve)));
  const address = server.address();
  assert.notEqual(address, null);
  assert.equal(typeof address, 'object');
  const cacheRoot = path.join(fixture.fixtureRoot, 'concurrent-cache');
  const source = {
    ...fixture.source,
    url: `http://127.0.0.1:${address.port}/sherpa-android.tar.bz2`,
  };

  const results = await Promise.all([
    runSherpaPrepareProcess(source, cacheRoot),
    runSherpaPrepareProcess(source, cacheRoot),
  ]);

  for (const result of results) {
    assert.equal(result.status, 0, result.stderr || result.stdout);
  }
  const versionRoot = path.join(cacheRoot, fixture.source.version);
  assert.ok(requests >= 1 && requests <= results.length, `unexpected request count: ${requests}`);
  const runtimeRoots = fs.readdirSync(versionRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name.startsWith('runtime-'))
    .map((entry) => path.join(versionRoot, entry.name));
  assert.ok(runtimeRoots.length >= 1 && runtimeRoots.length <= results.length);
  for (const runtimeRoot of runtimeRoots) {
    assert.equal(fs.existsSync(path.join(runtimeRoot, '.complete.json')), true);
    assert.equal(
      fs.existsSync(path.join(runtimeRoot, 'jniLibs', 'arm64-v8a', 'libonnxruntime.so')),
      true,
    );
  }
});

test('Android sherpa runtime atomic publication ignores orphaned lock metadata', async () => {
  const { prepareAndroidSherpaRuntime } = await loadAndroidSherpaRuntime();
  const fixture = createSherpaArchiveFixture();
  const cacheRoot = path.join(fixture.fixtureRoot, 'orphaned-lock-cache');
  const versionRoot = path.join(cacheRoot, fixture.source.version);
  fs.mkdirSync(versionRoot, { recursive: true });
  for (const fileName of ['prepare.lock', 'prepare.recovery.lock']) {
    fs.writeFileSync(path.join(versionRoot, fileName), JSON.stringify({
      pid: process.pid,
      token: `orphaned-${fileName}`,
      createdAt: Date.now(),
    }));
  }

  const prepared = await prepareAndroidSherpaRuntime({
    source: fixture.source,
    cacheRoot,
    selectedAbis: ['arm64-v8a'],
    archiveOverride: fixture.archivePath,
  });

  assert.equal(
    fs.existsSync(path.join(prepared.rootDir, 'jniLibs', 'arm64-v8a', 'libonnxruntime.so')),
    true,
  );
});

test('Android sherpa runtime loads and validates its checked-in source lock', async () => {
  const { loadAndroidSherpaSource } = await loadAndroidSherpaRuntime();
  const source = loadAndroidSherpaSource(path.join(
    repoRoot,
    'platforms',
    'android',
    'packaging',
    'sherpa-onnx-sources.json',
  ));

  assert.equal(source.version, '1.13.4');
  assert.equal(source.sha256, '7983fc3de23f6e64148f2fb05fa94a2efaa8c0516cc1573383dc5c7d4d2a43b0');
  assert.deepEqual(source.runtimeLibraries, [
    'libsherpa-onnx-c-api.so',
    'libonnxruntime.so',
  ]);
});

test('Android sherpa runtime source locks reject non-HTTPS download URLs', async () => {
  const { loadAndroidSherpaSource } = await loadAndroidSherpaRuntime();
  const fixture = createSherpaArchiveFixture();
  const sourceLockPath = path.join(fixture.fixtureRoot, 'source-lock.json');
  fs.writeFileSync(sourceLockPath, JSON.stringify({
    ...fixture.source,
    url: 'http://downloads.example.test/sherpa-android.tar.bz2',
  }));

  assert.throws(
    () => loadAndroidSherpaSource(sourceLockPath),
    /HTTPS URL is required/u,
  );
});

test('Android sherpa runtime stages only the Rust binding native dependencies', async () => {
  const { prepareAndroidSherpaRuntime, stageAndroidSherpaRuntime } = await loadAndroidSherpaRuntime();
  const fixture = createSherpaArchiveFixture();
  const prepared = await prepareAndroidSherpaRuntime({
    source: fixture.source,
    cacheRoot: path.join(fixture.fixtureRoot, 'cache'),
    selectedAbis: ['arm64-v8a'],
    archiveOverride: fixture.archivePath,
  });
  const sourceAbiDir = path.join(prepared.rootDir, 'jniLibs', 'arm64-v8a');
  fs.writeFileSync(path.join(sourceAbiDir, 'libsherpa-onnx-jni.so'), 'not staged');
  fs.writeFileSync(path.join(sourceAbiDir, 'libsherpa-onnx-cxx-api.so'), 'not staged');
  const outputDir = path.join(fixture.fixtureRoot, 'jni-output');

  stageAndroidSherpaRuntime(prepared, 'arm64-v8a', outputDir);

  assert.deepEqual(fs.readdirSync(path.join(outputDir, 'arm64-v8a')).sort(), [
    'libonnxruntime.so',
    'libsherpa-onnx-c-api.so',
  ]);
});
