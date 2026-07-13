import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { pathToFileURL } from 'node:url';
import {
  assertAndroidRecoveryConsumerSmoke,
  assertAndroidRecoverySampleSmoke,
  assertAndroidStreamingSmoke,
  assertCargoDependencyVersionAndFeature,
  assertStreamingAsrArchitecture,
  exists,
  expectedUniffiErrorVariants,
  read,
  repoRoot,
} from './test-support/repository.js';
import { node, androidNdkAbiCases, androidNdkToolPaths, runAndroidNdkPrint } from './test-support/android-ndk-fixtures.js';

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

test('android uniffi sample publishes a consumable local Maven artifact', () => {
  const sampleLibraryGradle = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'sample-library', 'build.gradle.kts'),
    'utf8',
  );
  const verifierScript = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'verify-uniffi-android-sample.js'),
    'utf8',
  );
  const androidReadme = read('platforms', 'android', 'README.md');

  assert.match(sampleLibraryGradle, /id\("maven-publish"\)/u);
  assert.match(sampleLibraryGradle, /from\(components\["debug"\]\)/u);
  assert.match(sampleLibraryGradle, /groupId\s*=\s*"com\.sona"/u);
  assert.match(sampleLibraryGradle, /artifactId\s*=\s*"sona-uniffi-bindings"/u);
  assert.match(sampleLibraryGradle, /version\s*=\s*project\.version\.toString\(\)/u);
  assert.match(sampleLibraryGradle, /url\s*=\s*uri\(layout\.buildDirectory\.dir\("repo"\)\)/u);

  assert.match(verifierScript, /:sample-library:publishDebugPublicationToSonaAndroidSampleRepository/u);
  assert.match(verifierScript, /verifyAndroidSampleMavenPublication/u);
  assert.match(verifierScript, /sona-uniffi-bindings-\$\{samplePublicationVersion\}\.module/u);
  assert.match(verifierScript, /verifyAndroidSampleGradleModuleMetadata/u);
  assert.match(verifierScript, /net\.java\.dev\.jna:jna/u);
  assert.match(verifierScript, /org\.jetbrains\.kotlinx:kotlinx-coroutines-core/u);

  assert.match(androidReadme, /publishDebugPublicationToSonaAndroidSampleRepository/u);
});

test('android uniffi sample includes a separate Maven consumer module', () => {
  const settingsGradle = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'settings.gradle.kts'),
    'utf8',
  );
  const consumerGradlePath = path.join(
    repoRoot,
    'platforms',
    'android',
    'sample-consumer',
    'consumer-library',
    'build.gradle.kts',
  );
  const consumerSmokePath = path.join(
    repoRoot,
    'platforms',
    'android',
    'sample-consumer',
    'consumer-library',
    'src',
    'main',
    'kotlin',
    'com',
    'sona',
    'uniffi',
    'consumer',
    'SonaUniffiConsumerSmoke.kt',
  );
  const verifierScript = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'verify-uniffi-android-sample.js'),
    'utf8',
  );
  const androidReadme = read('platforms', 'android', 'README.md');

  assert.match(settingsGradle, /include\(":consumer-library"\)/u);
  assert.match(settingsGradle, /maven\s*\{\s*url\s*=\s*uri\("sample-library\/build\/repo"\)/u);

  assert.equal(fs.existsSync(consumerGradlePath), true);
  const consumerGradle = fs.readFileSync(consumerGradlePath, 'utf8');
  assert.match(consumerGradle, /id\("com\.android\.library"\)/u);
  assert.match(consumerGradle, /namespace\s*=\s*"com\.sona\.uniffi\.consumer"/u);
  assert.doesNotMatch(consumerGradle, /sona-uniffi-bindings\.gradle\.kts/u);
  assert.doesNotMatch(consumerGradle, /generateSonaUniffiKotlin|buildSonaUniffiAndroidLibraries/u);

  assert.equal(fs.existsSync(consumerSmokePath), true);
  const consumerSmoke = fs.readFileSync(consumerSmokePath, 'utf8');
  assert.match(consumerSmoke, /import com\.sona\.uniffi\.sample\.SonaUniffiSmoke/u);
  assert.match(consumerSmoke, /import uniffi\.sona_uniffi_bind\.defaultConfigJson/u);

  assert.match(verifierScript, /:consumer-library:assembleDebug/u);
  assert.match(verifierScript, /verifyAndroidConsumerAar/u);
  assert.match(verifierScript, /com\/sona\/uniffi\/consumer\/SonaUniffiConsumerSmoke/u);

  assert.match(androidReadme, /consumer-library/u);
});

test('android uniffi JNI staging clears stale ABI outputs before packaging', () => {
  const buildAndroidLibsScript = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'build-uniffi-android-libs.js'),
    'utf8',
  );
  const androidBindingsGradle = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sona-uniffi-bindings.gradle.kts'),
    'utf8',
  );

  assert.match(androidBindingsGradle, /jniLibs\.directories\.add\(generatedJniLibsDir\.get\(\)\.asFile\.path\)/u);
  assert.match(
    buildAndroidLibsScript,
    /function prepareOutputDirectory\(outDir\)\s*\{\s*fs\.rmSync\(outDir,\s*\{\s*recursive:\s*true,\s*force:\s*true\s*\}\);\s*fs\.mkdirSync\(outDir,\s*\{\s*recursive:\s*true\s*\}\);\s*\}/u,
  );
  assert.match(buildAndroidLibsScript, /if \(!dryRun && !printLinkerEnv\)\s*\{\s*prepareOutputDirectory\(outDir\);\s*\}/u);
});

test('core ASR request contract is exposed through TS and UniFFI binding crates', () => {
  const coreCargo = read('core', 'Cargo.toml');
  const tsBindLib = read('adapters', 'ts_bind', 'src', 'lib.rs');
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiAsrMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'asr_mapper.rs'),
    'utf8',
  );

  assert.match(coreCargo, /specta = \{[^}]*features = \["derive", "serde_json"\]/u);

  for (const typeName of [
    'AsrTranscriptionRequest',
    'AsrEngineConfig',
    'OnlineAsrProviderRequest',
    'VolcengineDoubaoAsrConfig',
    'TranscriptPostprocessOptions',
    'SpeakerProcessingConfig',
    'ModelFileConfig',
  ]) {
    assert.match(tsBindLib, new RegExp(`\\b${typeName}\\b`, 'u'));
  }

  assert.match(uniffiLib, /default_batch_segmentation_mode/u);
  assert.match(uniffiLib, /online_asr_provider_request/u);
  assert.match(uniffiLib, /volcengine_doubao_asr_config_from_json/u);
  assert.match(uniffiAsrMapper, /pub enum FfiAsrEngine/u);
  assert.match(uniffiAsrMapper, /pub enum FfiAsrMode/u);
  assert.match(uniffiAsrMapper, /pub enum FfiBatchSegmentationMode/u);
  assert.match(uniffiAsrMapper, /pub struct FfiOnlineAsrProviderRequest/u);
  assert.match(uniffiAsrMapper, /pub struct FfiVolcengineDoubaoAsrConfig/u);
});

test('core online ASR provider manifest is exposed through UniFFI for mobile bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiAsrBridge = read('adapters', 'uniffi_bind', 'src', 'asr_bridge.rs');
  const uniffiAsrMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'asr_mapper.rs'),
    'utf8',
  );

  assert.match(uniffiAsrBridge, /online_asr_providers as core_online_asr_providers/u);
  assert.match(uniffiAsrBridge, /find_online_asr_provider as core_find_online_asr_provider/u);
  assert.match(uniffiLib, /pub fn online_asr_providers\(\) -> Vec<FfiOnlineAsrProvider>/u);
  assert.match(
    uniffiLib,
    /pub fn find_online_asr_provider\(\s*provider_id: String,?\s*\) -> Option<FfiOnlineAsrProvider>/u,
  );
  assert.match(uniffiAsrBridge, /core_online_asr_providers\(\)\s*\.iter\(\)\s*\.map\(mapper::online_asr_provider_to_ffi\)/u);
  assert.match(uniffiAsrBridge, /core_find_online_asr_provider\(&provider_id\)\s*\.map\(mapper::online_asr_provider_to_ffi\)/u);

  for (const typeName of [
    'FfiOnlineAsrProvider',
    'FfiOnlineAsrCapability',
    'FfiOnlineAsrBatchCapability',
    'FfiOnlineAsrLocalFileBatchMode',
  ]) {
    assert.match(uniffiAsrMapper, new RegExp(`pub struct ${typeName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`\\b${typeName}\\b`, 'u'));
  }
  assert.match(uniffiAsrMapper, /pub fn online_asr_provider_to_ffi/u);
});

test('UniFFI ASR bridge is isolated from the top-level binding facade', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const uniffiAsrBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'asr_bridge.rs');

  assert.match(uniffiLib, /^mod asr_bridge;/mu);
  assert.equal(fs.existsSync(uniffiAsrBridgePath), true);
  assert.match(uniffiLib, /SonaCoreFacade::default_batch_segmentation_mode\(\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::online_asr_providers\(\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::online_asr_provider_request\(provider_id, profile_id, config_json\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::volcengine_doubao_asr_config_from_json\(config_json\)/u);
  assert.doesNotMatch(uniffiLib, /asr_bridge::/u);
  assert.doesNotMatch(uniffiLib, /online_asr_providers as core_online_asr_providers/u);
  assert.doesNotMatch(uniffiLib, /find_online_asr_provider as core_find_online_asr_provider/u);
  assert.doesNotMatch(uniffiLib, /serde_json::from_str\(&config_json\)/u);
  assert.match(uniffiFacade, /asr_bridge::default_batch_segmentation_mode\(\)/u);
  assert.match(uniffiFacade, /asr_bridge::online_asr_providers\(\)/u);
  assert.match(uniffiFacade, /asr_bridge::online_asr_provider_request\(provider_id, profile_id, config_json\)/u);
  assert.match(uniffiFacade, /asr_bridge::volcengine_doubao_asr_config_from_json\(config_json\)/u);

  const uniffiAsrBridge = fs.readFileSync(uniffiAsrBridgePath, 'utf8');
  assert.match(uniffiAsrBridge, /online_asr_providers as core_online_asr_providers/u);
  assert.match(uniffiAsrBridge, /find_online_asr_provider as core_find_online_asr_provider/u);
  assert.match(uniffiAsrBridge, /parse_core_json\(&config_json, "ASR provider config"\)/u);
  assert.match(uniffiAsrBridge, /pub\(crate\) fn volcengine_doubao_asr_config_from_json/u);
});

test('UniFFI runtime bridge is isolated from the top-level binding facade', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const uniffiRuntimeBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'runtime_bridge.rs');

  assert.match(uniffiLib, /^mod runtime_bridge;/mu);
  assert.equal(fs.existsSync(uniffiRuntimeBridgePath), true);
  assert.match(uniffiLib, /SonaCoreFacade::normalize_export_format\(value\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::runtime_path_status\(path\)/u);
  assert.doesNotMatch(uniffiLib, /runtime_bridge::/u);
  assert.doesNotMatch(uniffiLib, /use sona_core::export::ExportFormat/u);
  assert.doesNotMatch(uniffiLib, /use sona_runtime_fs::resolve_runtime_path_status/u);
  assert.match(uniffiFacade, /runtime_bridge::normalize_export_format\(value\)/u);
  assert.match(uniffiFacade, /runtime_bridge::runtime_path_status\(path\)/u);

  const uniffiRuntimeBridge = fs.readFileSync(uniffiRuntimeBridgePath, 'utf8');
  assert.match(uniffiRuntimeBridge, /use sona_core::export::ExportFormat/u);
  assert.match(uniffiRuntimeBridge, /use sona_runtime_fs::resolve_runtime_path_status/u);
  assert.match(uniffiRuntimeBridge, /mapper::runtime_path_status_to_ffi\(resolve_runtime_path_status\(&path\)\)/u);
  assert.match(uniffiRuntimeBridge, /pub\(crate\) fn normalize_export_format/u);
});

test('UniFFI facade delegates bridge adapters outside the top-level export module', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacadePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'facade.rs');

  assert.match(uniffiLib, /^mod facade;/mu);
  assert.match(uniffiLib, /^pub use facade::SonaCoreFacade;/mu);
  assert.match(uniffiLib, /SonaCoreFacade::normalize_export_format\(value\)/u);
  assert.doesNotMatch(uniffiLib, /pub struct SonaCoreFacade/u);
  assert.doesNotMatch(uniffiLib, /impl SonaCoreFacade/u);
  assert.equal(fs.existsSync(uniffiFacadePath), true);

  const uniffiFacade = fs.readFileSync(uniffiFacadePath, 'utf8');
  assert.match(uniffiFacade, /pub struct SonaCoreFacade/u);
  assert.match(uniffiFacade, /impl SonaCoreFacade/u);
  assert.match(uniffiFacade, /runtime_bridge::normalize_export_format\(value\)/u);
  assert.match(uniffiFacade, /model_bridge::model_catalog_snapshot\(models_dir, installed_model_ids\)/u);
  assert.match(uniffiFacade, /llm_bridge::plan_summary_prompt_chunks_json/u);
  assert.match(uniffiFacade, /asr_bridge::online_asr_provider_request\(provider_id, profile_id, config_json\)/u);
  assert.match(uniffiFacade, /config_bridge::resolve_effective_config_json\(global_config_json, project_json\)/u);
  assert.doesNotMatch(uniffiFacade, /#\[uniffi::export\]/u);
});

test('UniFFI mapper facade re-exports focused domain mappers', () => {
  const mapperFacade = read('adapters', 'uniffi_bind', 'src', 'mapper.rs');

  for (const mapperName of [
    'runtime_mapper',
    'asr_mapper',
    'llm_mapper',
    'model_mapper',
    'config_mapper',
  ]) {
    assert.match(mapperFacade, new RegExp(`#\\[path = "mapper/${mapperName}\\.rs"\\]\\s*mod ${mapperName};`, 'u'));
    assert.match(mapperFacade, new RegExp(`pub use ${mapperName}::\\*;`, 'u'));
    assert.equal(
      exists('adapters', 'uniffi_bind', 'src', 'mapper', `${mapperName}.rs`),
      true,
    );
  }

  assert.doesNotMatch(mapperFacade, /use sona_core::/u);
  assert.doesNotMatch(mapperFacade, /pub (?:enum|struct) Ffi/u);
});

test('UniFFI root re-exports nested public model catalog records', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiModelMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'model_mapper.rs'),
    'utf8',
  );
  const mapperExports = uniffiLib.match(/pub use mapper::\{([\s\S]*?)\};/u)?.[1] ?? '';

  assert.match(uniffiModelMapper, /pub struct FfiModelCatalogGroup/u);
  assert.match(uniffiModelMapper, /pub groups: Vec<FfiModelCatalogGroup>/u);
  assert.match(mapperExports, /\bFfiModelCatalogGroup\b/u);
});

test('core LLM provider manifest is exposed through UniFFI for mobile bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiLlmBridge = read('adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');
  const uniffiLlmMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'llm_mapper.rs'),
    'utf8',
  );

  assert.match(uniffiLlmBridge, /llm_providers as core_llm_providers/u);
  assert.match(uniffiLlmBridge, /find_llm_provider_by_id_or_alias as core_find_llm_provider_by_id_or_alias/u);
  assert.match(uniffiLib, /pub fn llm_providers\(\) -> Vec<FfiLlmProvider>/u);
  assert.match(
    uniffiLib,
    /pub fn find_llm_provider_by_id_or_alias\(\s*id_or_alias: String,?\s*\) -> Option<FfiLlmProvider>/u,
  );
  assert.match(uniffiLlmBridge, /core_llm_providers\(\)\s*\.iter\(\)\s*\.map\(mapper::llm_provider_to_ffi\)/u);
  assert.match(
    uniffiLlmBridge,
    /core_find_llm_provider_by_id_or_alias\(&id_or_alias\)\s*\.map\(mapper::llm_provider_to_ffi\)/u,
  );

  assert.match(uniffiLlmMapper, /pub struct FfiLlmProviderDefaults/u);
  assert.match(uniffiLlmMapper, /pub struct FfiLlmProvider/u);
  assert.match(uniffiLlmMapper, /pub fn llm_provider_to_ffi/u);
});

test('core LLM request contracts are exposed through UniFFI binding records', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiLlmMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'llm_mapper.rs'),
    'utf8',
  );

  for (const exportName of [
    'llm_config_from_json',
    'polish_segments_request_from_json',
    'translate_segments_request_from_json',
    'summarize_transcript_request_from_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}`, 'u'));
  }

  for (const typeName of [
    'FfiLlmProviderStrategy',
    'FfiLlmConfig',
    'FfiLlmSegmentInput',
    'FfiSummarySegmentInput',
    'FfiSummaryTemplateConfig',
    'FfiPolishSegmentsRequest',
    'FfiTranslateSegmentsRequest',
    'FfiSummarizeTranscriptRequest',
  ]) {
    assert.match(uniffiLlmMapper, new RegExp(`pub (?:enum|struct) ${typeName}`, 'u'));
  }

  assert.match(uniffiLlmMapper, /pub fn llm_config_to_ffi/u);
  assert.match(uniffiLlmMapper, /pub fn polish_segments_request_to_ffi/u);
  assert.match(uniffiLlmMapper, /pub fn translate_segments_request_to_ffi/u);
  assert.match(uniffiLlmMapper, /pub fn summarize_transcript_request_to_ffi/u);
});

test('UniFFI LLM JSON bridge is isolated from the top-level binding facade', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const uniffiLlmBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');

  assert.match(uniffiLib, /^mod llm_bridge;/mu);
  assert.equal(fs.existsSync(uniffiLlmBridgePath), true);
  assert.match(uniffiLib, /SonaCoreFacade::validate_llm_config_json\(config_json\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::llm_segment_inputs_from_transcript_json\(segments_json\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::plan_polish_prompt_chunks_json\(/u);
  assert.doesNotMatch(uniffiLib, /llm_bridge::/u);
  assert.doesNotMatch(uniffiLib, /validate_llm_config as core_validate_llm_config/u);
  assert.doesNotMatch(uniffiLib, /segment_inputs_from_transcript as core_segment_inputs_from_transcript/u);
  assert.doesNotMatch(uniffiLib, /plan_segment_task_chunks as core_plan_segment_task_chunks/u);
  assert.match(uniffiFacade, /llm_bridge::validate_llm_config_json\(config_json\)/u);
  assert.match(uniffiFacade, /llm_bridge::llm_segment_inputs_from_transcript_json\(segments_json\)/u);
  assert.match(uniffiFacade, /llm_bridge::plan_polish_prompt_chunks_json\(/u);

  const uniffiLlmBridge = fs.readFileSync(uniffiLlmBridgePath, 'utf8');
  assert.match(uniffiLlmBridge, /use crate::json_bridge::\{[\s\S]*map_core_validation_result[\s\S]*parse_core_json[\s\S]*serialize_core_json[\s\S]*\};/u);
  assert.match(uniffiLlmBridge, /validate_llm_config as core_validate_llm_config/u);
  assert.match(uniffiLlmBridge, /segment_inputs_from_transcript as core_segment_inputs_from_transcript/u);
  assert.match(uniffiLlmBridge, /plan_segment_task_chunks as core_plan_segment_task_chunks/u);
  assert.match(uniffiLlmBridge, /pub\(crate\) fn plan_polish_prompt_chunks_json/u);
});

test('core LLM request validation is exposed through UniFFI JSON bridge', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiJsonBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'json_bridge.rs');
  const uniffiLlmBridge = read('adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');

  for (const validationName of [
    'validate_llm_config_json',
    'validate_llm_generate_request_json',
    'validate_polish_segments_request_json',
    'validate_translate_segments_request_json',
    'validate_summarize_transcript_request_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${validationName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${validationName}`, 'u'));
  }

  for (const coreValidationName of [
    'validate_llm_config',
    'validate_llm_generate_request',
    'validate_polish_segments_request',
    'validate_translate_segments_request',
    'validate_summarize_transcript_request',
  ]) {
    assert.match(uniffiLlmBridge, new RegExp(`${coreValidationName} as core_${coreValidationName}`, 'u'));
    assert.match(uniffiLlmBridge, new RegExp(`core_${coreValidationName}\\(&`, 'u'));
  }

  assert.match(uniffiLib, /^mod json_bridge;/mu);
  assert.doesNotMatch(uniffiLib, /use json_bridge::/u);
  assert.match(uniffiLlmBridge, /use crate::json_bridge::\{[\s\S]*map_core_validation_result[\s\S]*parse_core_json[\s\S]*serialize_core_json[\s\S]*\};/u);
  assert.doesNotMatch(uniffiLib, /fn map_core_validation_result\(result: Result<\(\), String>\) -> SonaCoreBindingResult<\(\)>/u);
  assert.equal(fs.existsSync(uniffiJsonBridgePath), true);
  const uniffiJsonBridge = fs.readFileSync(uniffiJsonBridgePath, 'utf8');
  assert.match(uniffiJsonBridge, /pub\(crate\) fn map_core_validation_result\([\s\S]*result: Result<\(\), String>[\s\S]*\) -> SonaCoreBindingResult<\(\)>/u);
  assert.match(uniffiJsonBridge, /pub\(crate\) fn parse_core_json</u);
  assert.match(uniffiJsonBridge, /pub\(crate\) fn parse_optional_core_json</u);
  assert.match(uniffiJsonBridge, /pub\(crate\) fn serialize_core_json</u);
});

test('core transcript LLM job helpers are exposed through UniFFI JSON bridge', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiLlmBridge = read('adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');

  for (const exportName of [
    'llm_segment_inputs_from_transcript_json',
    'summary_segment_inputs_from_transcript_json',
    'merge_translated_items_into_transcript_json',
    'merge_polished_items_into_transcript_json',
    'summary_source_fingerprint_from_transcript_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}`, 'u'));
  }

  for (const coreHelperName of [
    'segment_inputs_from_transcript',
    'summary_inputs_from_transcript',
    'merge_translated_items_into_segments',
    'merge_polished_items_into_segments',
    'compute_summary_source_fingerprint',
  ]) {
    assert.match(uniffiLlmBridge, new RegExp(`${coreHelperName} as core_${coreHelperName}`, 'u'));
    assert.match(uniffiLlmBridge, new RegExp(`core_${coreHelperName}\\(`, 'u'));
  }

  assert.match(uniffiLlmBridge, /parse_core_json\(&segments_json, "transcript segments"\)/u);
  assert.match(uniffiLlmBridge, /serialize_core_json\(&merged, "merged transcript segments"\)/u);
});

test('core LLM prompt and chunk parsing helpers are exposed through UniFFI JSON bridge', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiLlmBridge = read('adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');
  const uniffiLlmMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'llm_mapper.rs'),
    'utf8',
  );

  for (const exportName of [
    'build_polish_prompt_json',
    'build_translate_prompt_json',
    'build_summary_chunk_prompt_json',
    'build_summary_finalize_prompt_json',
    'parse_polish_chunk_json',
    'parse_translate_chunk_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}`, 'u'));
  }

  for (const coreHelperName of [
    'build_polish_prompt',
    'build_translate_prompt',
    'build_summary_chunk_prompt',
    'build_summary_finalize_prompt',
    'parse_polish_chunk',
    'parse_translate_chunk',
  ]) {
    assert.match(uniffiLlmBridge, new RegExp(`${coreHelperName} as core_${coreHelperName}`, 'u'));
    assert.match(uniffiLlmBridge, new RegExp(`core_${coreHelperName}\\(`, 'u'));
  }

  for (const typeName of [
    'FfiPolishedSegment',
    'FfiTranslatedSegment',
  ]) {
    assert.match(uniffiLlmMapper, new RegExp(`pub struct ${typeName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`\\b${typeName}\\b`, 'u'));
  }

  assert.match(uniffiLlmMapper, /pub fn polished_segment_to_ffi/u);
  assert.match(uniffiLlmMapper, /pub fn translated_segment_to_ffi/u);
  assert.match(uniffiLlmBridge, /parse_core_json\(&segments_json, "LLM segment inputs"\)/u);
  assert.match(uniffiLlmBridge, /parse_core_json\(&template_json, "summary template"\)/u);
});

test('core LLM prompt chunk planning is exposed through UniFFI JSON bridge', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiLlmBridge = read('adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');
  const uniffiLlmMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'llm_mapper.rs'),
    'utf8',
  );

  for (const exportName of [
    'plan_polish_prompt_chunks_json',
    'plan_translate_prompt_chunks_json',
    'plan_summary_prompt_chunks_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}`, 'u'));
  }

  assert.match(uniffiLlmMapper, /pub struct FfiLlmPromptChunk/u);
  assert.match(uniffiLlmMapper, /pub fn llm_prompt_chunk_to_ffi/u);
  assert.match(uniffiLlmBridge, /plan_segment_task_chunks as core_plan_segment_task_chunks/u);
  assert.match(uniffiLlmBridge, /split_summary_segments as core_split_summary_segments/u);
  assert.match(uniffiLlmBridge, /core_plan_segment_task_chunks\(/u);
  assert.match(uniffiLlmBridge, /core_split_summary_segments\(/u);
  assert.match(uniffiLlmBridge, /DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET/u);
  assert.match(uniffiLlmBridge, /DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET/u);
  assert.match(uniffiLlmBridge, /MIN_SUMMARY_CHUNK_CHAR_BUDGET/u);
});

test('core config migration surface is exposed through UniFFI JSON bridge', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiConfigBridge = read('adapters', 'uniffi_bind', 'src', 'config_bridge.rs');
  const uniffiConfigMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'config_mapper.rs'),
    'utf8',
  );

  for (const exportName of [
    'default_config_json',
    'migrate_app_config_json',
    'resolve_effective_config_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}`, 'u'));
  }

  assert.match(uniffiConfigBridge, /core_migrate_app_config/u);
  assert.match(uniffiConfigBridge, /core_resolve_effective_config/u);
  assert.match(uniffiConfigMapper, /pub struct FfiConfigMigrationResult/u);
  assert.match(uniffiConfigMapper, /pub config_json: String/u);
  assert.match(uniffiConfigMapper, /pub migrated: bool/u);
});

test('app config repository persistence is exposed through UniFFI and Android bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const repositoryBridgePath = path.join(
    repoRoot,
    'adapters',
    'uniffi_bind',
    'src',
    'app_config_repository_bridge.rs',
  );
  const repositoryBridge = fs.readFileSync(repositoryBridgePath, 'utf8');
  const androidSample = read(
    'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt',
  );
  const androidConsumer = read(
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt',
  );
  const exports = [
    'load_app_config_json',
    'save_app_config_json',
    'get_app_setting_json',
    'set_app_setting_json',
  ];

  assert.match(uniffiLib, /^mod app_config_repository_bridge;/mu);
  assert.match(uniffiLib, /ConfigRepository\s*\{\s*reason: String\s*\}/u);
  const errorBody = /pub enum SonaCoreBindingError\s*\{([\s\S]*?)\n\}/u
    .exec(uniffiLib)?.[1] ?? assert.fail('missing SonaCoreBindingError');
  const errorVariants = [
    ...errorBody.matchAll(/^\s*([A-Z][A-Za-z0-9_]*)\s*\{/gmu),
  ].map((match) => match[1]);
  assert.deepEqual(errorVariants, expectedUniffiErrorVariants);
  for (const exportName of exports) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}\\(`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}\\(`, 'u'));
    assert.match(uniffiFacade, new RegExp(`pub fn ${exportName}\\(`, 'u'));
    assert.match(
      uniffiFacade,
      new RegExp(`app_config_repository_bridge::${exportName}\\(`, 'u'),
    );
    assert.match(repositoryBridge, new RegExp(`pub\\(crate\\) fn ${exportName}\\(`, 'u'));
  }

  assert.match(androidSample, /^import uniffi\.sona_uniffi_bind\.loadAppConfigJson$/mu);
  assert.match(androidSample, /^import uniffi\.sona_uniffi_bind\.saveAppConfigJson$/mu);
  assert.match(androidSample, /loadAppConfigJson\(appDataDir\)/u);
  assert.match(androidSample, /saveAppConfigJson\(appDataDir, configJson\)/u);
  assert.match(androidConsumer, /^import uniffi\.sona_uniffi_bind\.loadAppConfigJson$/mu);
  assert.match(androidConsumer, /loadAppConfigJson\(appDataDir\)/u);
});

test('dashboard reporting is exposed through UniFFI and Android bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const dashboardBridge = read('adapters', 'uniffi_bind', 'src', 'dashboard_bridge.rs');
  const androidSample = read(
    'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt',
  );
  const androidConsumer = read(
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt',
  );

  assert.match(uniffiLib, /^mod dashboard_bridge;/mu);
  assert.match(
    uniffiLib,
    /#\[uniffi::export\]\s*pub async fn load_dashboard_snapshot_json\(\s*app_data_dir: String,\s*deep: bool,\s*\) -> SonaCoreBindingResult<String>/u,
  );
  assert.match(uniffiLib, /SonaCoreFacade::load_dashboard_snapshot_json\(app_data_dir, deep\)\.await/u);
  assert.match(uniffiFacade, /pub async fn load_dashboard_snapshot_json\(/u);
  assert.match(uniffiFacade, /dashboard_bridge::load_dashboard_snapshot_json\(app_data_dir, deep\)\.await/u);
  assert.match(dashboardBridge, /tokio::task::spawn_blocking/u);
  assert.match(dashboardBridge, /Database::open_read_only_with_analytics/u);
  assert.match(dashboardBridge, /DashboardService::new/u);
  assert.match(dashboardBridge, /sona_runtime_fs::dashboard_snapshot_time_now\(\)/u);

  const errorBody = /pub enum SonaCoreBindingError\s*\{([\s\S]*?)\n\}/u
    .exec(uniffiLib)?.[1] ?? assert.fail('missing SonaCoreBindingError');
  const errorVariants = [
    ...errorBody.matchAll(/^\s*([A-Z][A-Za-z0-9_]*)\s*\{/gmu),
  ].map((match) => match[1]);
  assert.deepEqual(errorVariants, expectedUniffiErrorVariants);

  assert.match(androidSample, /^import uniffi\.sona_uniffi_bind\.loadDashboardSnapshotJson$/mu);
  assert.match(androidSample, /suspend fun loadDashboard\(appDataDir: String, deep: Boolean\): String/u);
  assert.match(androidSample, /loadDashboardSnapshotJson\(appDataDir, deep\)/u);
  assert.match(androidConsumer, /^import uniffi\.sona_uniffi_bind\.loadDashboardSnapshotJson$/mu);
  assert.match(androidConsumer, /suspend fun loadDashboard\(appDataDir: String\): String/u);
  assert.match(androidConsumer, /loadDashboardSnapshotJson\(appDataDir, false\)/u);
});

test('diagnostics snapshots are exposed through UniFFI and Android bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const diagnosticsBridge = read('adapters', 'uniffi_bind', 'src', 'diagnostics_bridge.rs');
  const androidSample = read(
    'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt',
  );
  const androidConsumer = read(
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt',
  );

  assert.match(uniffiLib, /^mod diagnostics_bridge;/mu);
  assert.match(
    uniffiLib,
    /#\[uniffi::export\]\s*pub async fn load_diagnostics_snapshot_json\(\s*app_data_dir: String,\s*input_json: String,\s*\) -> SonaCoreBindingResult<String>/u,
  );
  assert.match(uniffiLib, /SonaCoreFacade::load_diagnostics_snapshot_json\(app_data_dir, input_json\)\.await/u);
  assert.match(uniffiFacade, /pub async fn load_diagnostics_snapshot_json\(/u);
  assert.match(diagnosticsBridge, /tokio::task::spawn_blocking/u);
  assert.match(diagnosticsBridge, /FsDiagnosticsEnrichmentRepository::new/u);
  assert.match(diagnosticsBridge, /DiagnosticsService::new/u);
  assert.match(diagnosticsBridge, /diagnostics_scanned_at_now\(\)/u);
  assert.match(diagnosticsBridge, /serde_json::to_string\(&canonical\)/u);
  assert.doesNotMatch(diagnosticsBridge, /sona_local_asr|AsrState/u);

  assert.match(androidSample, /^import uniffi\.sona_uniffi_bind\.loadDiagnosticsSnapshotJson$/mu);
  assert.match(androidSample, /suspend fun loadDiagnostics\(appDataDir: String, inputJson: String\): String/u);
  assert.match(androidSample, /loadDiagnosticsSnapshotJson\(appDataDir, inputJson\)/u);
  assert.match(androidConsumer, /^import uniffi\.sona_uniffi_bind\.loadDiagnosticsSnapshotJson$/mu);
  assert.match(androidConsumer, /suspend fun loadDiagnostics\(appDataDir: String, inputJson: String\): String/u);
  assert.match(androidConsumer, /loadDiagnosticsSnapshotJson\(appDataDir, inputJson\)/u);
});

test('storage usage reporting is exposed through UniFFI and Android bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const storageBridge = read('adapters', 'uniffi_bind', 'src', 'storage_usage_bridge.rs');
  const androidSample = read(
    'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt',
  );
  const androidConsumer = read(
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt',
  );

  assert.match(uniffiLib, /^mod storage_usage_bridge;/mu);
  assert.match(
    uniffiLib,
    /#\[uniffi::export\]\s*pub async fn load_storage_usage_snapshot_json\(\s*app_data_dir: String,\s*\) -> SonaCoreBindingResult<String>/u,
  );
  assert.match(uniffiLib, /SonaCoreFacade::load_storage_usage_snapshot_json\(app_data_dir\)\.await/u);
  assert.match(uniffiFacade, /pub async fn load_storage_usage_snapshot_json\(/u);
  assert.match(storageBridge, /tokio::task::spawn_blocking/u);
  assert.match(storageBridge, /Database::open_read_only_with_analytics/u);
  assert.match(storageBridge, /SqliteStorageUsageRepository::new/u);
  assert.match(storageBridge, /StorageUsageService::new/u);
  assert.match(storageBridge, /storage_usage_generated_at_now\(\)/u);

  assert.match(androidSample, /^import uniffi\.sona_uniffi_bind\.loadStorageUsageSnapshotJson$/mu);
  assert.match(androidSample, /suspend fun loadStorageUsage\(appDataDir: String\): String/u);
  assert.match(androidSample, /loadStorageUsageSnapshotJson\(appDataDir\)/u);
  assert.match(androidConsumer, /^import uniffi\.sona_uniffi_bind\.loadStorageUsageSnapshotJson$/mu);
  assert.match(androidConsumer, /suspend fun loadStorageUsage\(appDataDir: String\): String/u);
  assert.match(androidConsumer, /loadStorageUsageSnapshotJson\(appDataDir\)/u);
});

test('UniFFI config bridge is isolated from the top-level binding facade', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const uniffiConfigBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'config_bridge.rs');

  assert.match(uniffiLib, /^mod config_bridge;/mu);
  assert.equal(fs.existsSync(uniffiConfigBridgePath), true);
  assert.match(uniffiLib, /SonaCoreFacade::default_config_json\(\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::migrate_app_config_json\(/u);
  assert.match(uniffiLib, /SonaCoreFacade::resolve_effective_config_json\(global_config_json, project_json\)/u);
  assert.doesNotMatch(uniffiLib, /config_bridge::/u);
  assert.doesNotMatch(uniffiLib, /migrate_app_config as core_migrate_app_config/u);
  assert.doesNotMatch(uniffiLib, /resolve_effective_config as core_resolve_effective_config/u);
  assert.doesNotMatch(uniffiLib, /parse_optional_core_json\(saved_config_json\.as_deref\(\), "saved config"\)/u);
  assert.match(uniffiFacade, /config_bridge::default_config_json\(\)/u);
  assert.match(uniffiFacade, /config_bridge::migrate_app_config_json\(/u);
  assert.match(uniffiFacade, /config_bridge::resolve_effective_config_json\(global_config_json, project_json\)/u);

  const uniffiConfigBridge = fs.readFileSync(uniffiConfigBridgePath, 'utf8');
  assert.match(uniffiConfigBridge, /default_config/u);
  assert.match(uniffiConfigBridge, /migrate_app_config as core_migrate_app_config/u);
  assert.match(uniffiConfigBridge, /resolve_effective_config as core_resolve_effective_config/u);
  assert.match(uniffiConfigBridge, /parse_optional_core_json\(saved_config_json\.as_deref\(\), "saved config"\)/u);
  assert.match(uniffiConfigBridge, /pub\(crate\) fn resolve_effective_config_json/u);
});

test('UniFFI model bridge is isolated from the top-level binding facade', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const uniffiModelBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'model_bridge.rs');

  assert.match(uniffiLib, /^mod model_bridge;/mu);
  assert.equal(fs.existsSync(uniffiModelBridgePath), true);
  assert.match(uniffiLib, /SonaCoreFacade::preset_models\(\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::model_catalog_selected_ids\(/u);
  assert.match(uniffiLib, /SonaCoreFacade::resolve_model_download\(model_id, models_dir\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::resolve_gpu_acceleration\(value\)/u);
  assert.doesNotMatch(uniffiLib, /model_bridge::/u);
  assert.doesNotMatch(uniffiLib, /preset_models as core_preset_models/u);
  assert.doesNotMatch(uniffiLib, /resolve_model_download as core_resolve_model_download/u);
  assert.doesNotMatch(uniffiLib, /resolve_gpu_acceleration as core_resolve_gpu_acceleration/u);
  assert.match(uniffiFacade, /model_bridge::preset_models\(\)/u);
  assert.match(uniffiFacade, /model_bridge::model_catalog_selected_ids\(/u);
  assert.match(uniffiFacade, /model_bridge::resolve_model_download\(model_id, models_dir\)/u);
  assert.match(uniffiFacade, /model_bridge::resolve_gpu_acceleration\(value\)/u);

  const uniffiModelBridge = fs.readFileSync(uniffiModelBridgePath, 'utf8');
  assert.match(uniffiModelBridge, /preset_models as core_preset_models/u);
  assert.match(uniffiModelBridge, /resolve_model_catalog_selected_ids/u);
  assert.match(uniffiModelBridge, /resolve_model_download as core_resolve_model_download/u);
  assert.match(uniffiModelBridge, /resolve_gpu_acceleration as core_resolve_gpu_acceleration/u);
  assert.match(uniffiModelBridge, /pub\(crate\) fn model_catalog_snapshot/u);
});

test('core model catalog selected id resolution is exposed through UniFFI bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiModelBridge = read('adapters', 'uniffi_bind', 'src', 'model_bridge.rs');
  const uniffiModelMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'model_mapper.rs'),
    'utf8',
  );

  assert.match(uniffiModelBridge, /resolve_model_catalog_selected_ids/u);
  assert.match(uniffiLib, /pub fn model_catalog_selected_ids/u);
  assert.match(uniffiLib, /SonaCoreFacade::model_catalog_selected_ids/u);
  assert.match(uniffiModelMapper, /pub struct FfiModelSelectionPaths/u);
  assert.match(uniffiModelMapper, /pub struct FfiModelCatalogSelectedIds/u);
  assert.match(uniffiModelMapper, /pub fn model_selection_paths_from_ffi/u);
  assert.match(uniffiModelMapper, /pub fn model_catalog_selected_ids_to_ffi/u);
});

test('core model catalog grouping and dependency requests are exposed through UniFFI snapshot', () => {
  const uniffiModelMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'model_mapper.rs'),
    'utf8',
  );

  for (const typeName of [
    'FfiModelCatalogSectionType',
    'FfiModelCatalogGroup',
    'FfiModelCatalogSection',
    'FfiModelDependencyConfigKey',
    'FfiModelDependencyRequest',
    'FfiModelDependencyRequestsForModel',
  ]) {
    assert.match(uniffiModelMapper, new RegExp(`pub (?:enum|struct) ${typeName}`, 'u'));
  }

  assert.match(uniffiModelMapper, /pub sections: Vec<FfiModelCatalogSection>/u);
  assert.match(uniffiModelMapper, /pub dependency_requests_by_model_id: Vec<FfiModelDependencyRequestsForModel>/u);
  assert.match(uniffiModelMapper, /pub fn model_catalog_section_to_ffi/u);
  assert.match(uniffiModelMapper, /pub fn model_dependency_requests_by_model_id_to_ffi/u);
});

test('core model catalog path indexes are exposed through UniFFI snapshot', () => {
  const uniffiModelMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'model_mapper.rs'),
    'utf8',
  );

  for (const typeName of [
    'FfiModelPathByIdEntry',
    'FfiModelIdByNormalizedPathEntry',
    'FfiModelCatalogPathMatchToken',
  ]) {
    assert.match(uniffiModelMapper, new RegExp(`pub struct ${typeName}`, 'u'));
  }

  assert.match(uniffiModelMapper, /pub model_path_by_id: Vec<FfiModelPathByIdEntry>/u);
  assert.match(uniffiModelMapper, /pub model_id_by_normalized_path: Vec<FfiModelIdByNormalizedPathEntry>/u);
  assert.match(uniffiModelMapper, /pub path_match_tokens: Vec<FfiModelCatalogPathMatchToken>/u);
  assert.match(uniffiModelMapper, /pub fn model_path_by_id_to_ffi/u);
  assert.match(uniffiModelMapper, /pub fn model_id_by_normalized_path_to_ffi/u);
  assert.match(uniffiModelMapper, /pub fn model_catalog_path_match_token_to_ffi/u);
});

test('core model download planning is exposed through UniFFI bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiModelBridge = read('adapters', 'uniffi_bind', 'src', 'model_bridge.rs');
  const uniffiModelMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'model_mapper.rs'),
    'utf8',
  );

  assert.match(uniffiModelBridge, /core_resolve_model_download/u);
  assert.match(uniffiModelBridge, /required_companion_models/u);
  assert.match(uniffiLib, /pub fn resolve_model_download/u);
  assert.match(uniffiLib, /SonaCoreFacade::resolve_model_download/u);
  assert.match(uniffiModelMapper, /pub struct FfiRequiredCompanionModels/u);
  assert.match(uniffiModelMapper, /pub struct FfiResolvedModelDownload/u);
  assert.match(uniffiModelMapper, /pub fn resolved_model_download_to_ffi/u);
});

test('core GPU acceleration config validation is exposed through UniFFI bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiModelBridge = read('adapters', 'uniffi_bind', 'src', 'model_bridge.rs');

  assert.match(uniffiModelBridge, /resolve_gpu_acceleration as core_resolve_gpu_acceleration/u);
  assert.match(uniffiLib, /pub fn resolve_gpu_acceleration/u);
  assert.match(uniffiLib, /SonaCoreFacade::resolve_gpu_acceleration/u);
});

test('UniFFI Kotlin bindings are generated through the 0.32 Android Gradle integration', () => {
  const workspaceCargo = read('Cargo.toml');
  const uniffiCargoPath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'Cargo.toml');
  const bindgenCargoPath = path.join(repoRoot, 'tools', 'uniffi_bindgen', 'Cargo.toml');
  const bindgenCargo = fs.readFileSync(
    bindgenCargoPath,
    'utf8',
  );
  const bindgenMain = read('tools', 'uniffi_bindgen', 'src', 'main.rs');
  const generateScript = read('scripts', 'generate-uniffi-kotlin.js');
  const gradleIntegration = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sona-uniffi-bindings.gradle.kts'),
    'utf8',
  );

  assert.match(workspaceCargo, /"tools\/uniffi_bindgen"/u);
  assertCargoDependencyVersionAndFeature(uniffiCargoPath, 'uniffi', '0.32', 'tokio');
  assert.match(bindgenCargo, /name\s*=\s*"sona-uniffi-bindgen"/u);
  assertCargoDependencyVersionAndFeature(bindgenCargoPath, 'uniffi', '0.32', 'cli');
  assert.match(bindgenMain, /uniffi::uniffi_bindgen_main\(\)/u);
  assert.match(generateScript, /cargo/u);
  assert.match(generateScript, /sona-uniffi-bind/u);
  assert.match(generateScript, /sona-uniffi-bindgen/u);
  assert.match(generateScript, /generate/u);
  assert.match(generateScript, /--library/u);
  assert.match(generateScript, /--language/u);
  assert.match(generateScript, /kotlin/u);
  assert.match(generateScript, /fs\.rmSync\(outDir,\s*\{\s*recursive:\s*true,\s*force:\s*true\s*\}\)/u);
  assert.match(gradleIntegration, /generateSonaUniffiKotlin/u);
  assert.match(gradleIntegration, /scripts\/generate-uniffi-kotlin\.js/u);
  assert.match(gradleIntegration, /generated\/source\/uniffi\/main\/kotlin/u);
  assert.match(gradleIntegration, /com\.android\.build\.api\.dsl\.LibraryExtension/u);
  assert.match(gradleIntegration, /java\.directories\.add\(generatedKotlinDir\.get\(\)\.asFile\.path\)/u);
  assert.match(gradleIntegration, /net\.java\.dev\.jna:jna:5\.12\.0@aar/u);
  assert.match(gradleIntegration, /org\.jetbrains\.kotlinx:kotlinx-coroutines-core:1\.6\.4/u);
});

test('UniFFI streaming ASR keeps its generated API online-only while linking the local runtime', () => {
  const uniffiCargoPath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'Cargo.toml');
  const streamingBridge = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'asr_streaming_bridge.rs'),
    'utf8',
  );

  assertStreamingAsrArchitecture(uniffiCargoPath, streamingBridge);
});

test('Android UniFFI streaming smoke compiles the generated observer and session surface', () => {
  const sampleKotlin = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin', 'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt'),
    'utf8',
  );
  const consumerKotlin = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin', 'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt'),
    'utf8',
  );

  for (const kotlinSmoke of [sampleKotlin, consumerKotlin]) {
    assertAndroidStreamingSmoke(kotlinSmoke);
  }
});

test('Android UniFFI recovery smoke calls generated snapshot bindings with app-data paths', () => {
  const sampleKotlin = read(
    'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt',
  );
  const consumerKotlin = read(
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt',
  );

  assertAndroidRecoverySampleSmoke(sampleKotlin);
  assertAndroidRecoveryConsumerSmoke(consumerKotlin);
});

test('Android UniFFI recovery guard rejects comment-only declarations and call-shaped strings', () => {
  const commentDecoy = `
    package com.sona.uniffi.sample
    // import uniffi.sona_uniffi_bind.loadRecoverySnapshotJson
    /*
    fun loadRecovery(appDataDir: String): String = loadRecoverySnapshotJson(appDataDir)
    */
  `;
  assert.throws(
    () => assertAndroidRecoveryConsumerSmoke(commentDecoy),
    /missing generated Kotlin import for loadRecoverySnapshotJson/u,
  );

  const stringDecoy = `
    package com.sona.uniffi.sample
    import uniffi.sona_uniffi_bind.loadRecoverySnapshotJson

    object SonaUniffiConsumerSmoke {
      fun loadRecovery(appDataDir: String): String {
        val unused = "loadRecoverySnapshotJson(appDataDir)"
        return appDataDir
      }
    }
  `;
  assert.throws(
    () => assertAndroidRecoveryConsumerSmoke(stringDecoy),
    /loadRecovery must directly delegate to the generated recovery binding/u,
  );

  const detachedDecoy = `
    package com.sona.uniffi.consumer
    import uniffi.sona_uniffi_bind.loadRecoverySnapshotJson

    object SonaUniffiConsumerSmoke

    fun loadRecovery(appDataDir: String): String = loadRecoverySnapshotJson(appDataDir)
  `;
  assert.throws(
    () => assertAndroidRecoveryConsumerSmoke(detachedDecoy),
    /SonaUniffiConsumerSmoke must contain loadRecovery/u,
  );

  const nestedDecoy = `
    package com.sona.uniffi.consumer
    import uniffi.sona_uniffi_bind.loadRecoverySnapshotJson

    object SonaUniffiConsumerSmoke {
      object Detached {
        fun loadRecovery(appDataDir: String): String = loadRecoverySnapshotJson(appDataDir)
      }
    }
  `;
  assert.throws(
    () => assertAndroidRecoveryConsumerSmoke(nestedDecoy),
    /missing loadRecovery Kotlin smoke method/u,
  );

  const deadBranchDecoy = `
    package com.sona.uniffi.consumer
    import uniffi.sona_uniffi_bind.loadRecoverySnapshotJson

    object SonaUniffiConsumerSmoke {
      fun loadRecovery(appDataDir: String): String {
        if (false) loadRecoverySnapshotJson(appDataDir)
        return appDataDir
      }
    }
  `;
  assert.throws(
    () => assertAndroidRecoveryConsumerSmoke(deadBranchDecoy),
    /loadRecovery must directly delegate to the generated recovery binding/u,
  );
});

test('UniFFI Android Gradle integration builds ABI-scoped native libraries', () => {
  const buildScript = read('scripts', 'build-uniffi-android-libs.js');
  const gradleIntegration = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sona-uniffi-bindings.gradle.kts'),
    'utf8',
  );
  const androidReadme = read('platforms', 'android', 'README.md');

  for (const [abi, target] of [
    ['arm64-v8a', 'aarch64-linux-android'],
    ['armeabi-v7a', 'armv7-linux-androideabi'],
    ['x86', 'i686-linux-android'],
    ['x86_64', 'x86_64-linux-android'],
  ]) {
    assert.match(buildScript, new RegExp(`['"]${abi}['"]:\\s*['"]${target}['"]`, 'u'));
  }

  assert.match(buildScript, /cargo/u);
  assert.match(buildScript, /build/u);
  assert.match(buildScript, /--target/u);
  assert.match(buildScript, /sona-uniffi-bind/u);
  assert.match(buildScript, /libsona_uniffi_bind\.so/u);
  assert.match(buildScript, /jniLibs/u);
  assert.match(buildScript, /SONA_ANDROID_ABIS/u);
  assert.match(buildScript, /SONA_ANDROID_MIN_SDK/u);
  assert.match(buildScript, /loadAndroidSherpaSource/u);
  assert.match(buildScript, /prepareAndroidSherpaRuntime/u);
  assert.match(buildScript, /stageAndroidSherpaRuntime/u);
  assert.match(buildScript, /sherpa-onnx-sources\.json/u);
  assert.match(buildScript, /SONA_SHERPA_ONNX_ANDROID_ARCHIVE/u);
  assert.match(buildScript, /SHERPA_ONNX_LIB_DIR/u);
  assert.match(gradleIntegration, /buildSonaUniffiAndroidLibraries/u);
  assert.match(gradleIntegration, /scripts\/build-uniffi-android-libs\.js/u);
  assert.match(gradleIntegration, /providers\.environmentVariable\("SONA_ANDROID_ABIS"\)/u);
  assert.match(gradleIntegration, /providers\.gradleProperty\("SONA_REPO_ROOT"\)/u);
  assert.match(gradleIntegration, /providers\.environmentVariable\("SONA_REPO_ROOT"\)/u);
  assert.match(gradleIntegration, /inputs\.property\("sonaAndroidAbis"/u);
  assert.match(gradleIntegration, /"--abis"/u);
  assert.match(gradleIntegration, /providers\.environmentVariable\("SONA_ANDROID_MIN_SDK"\)/u);
  assert.match(gradleIntegration, /providers\.environmentVariable\("SONA_SHERPA_ONNX_ANDROID_ARCHIVE"\)/u);
  assert.match(gradleIntegration, /if \(archive\.isAbsolute\) archive else File\(repoRoot, it\)/u);
  assert.match(
    gradleIntegration,
    /environment\("SONA_SHERPA_ONNX_ANDROID_ARCHIVE", sherpaArchive\.absolutePath\)/u,
  );
  assert.match(gradleIntegration, /inputs\.property\("sonaAndroidMinSdk"/u);
  assert.match(gradleIntegration, /"--min-sdk"/u);
  assert.match(gradleIntegration, /generated\/jniLibs\/main/u);
  assert.match(gradleIntegration, /jniLibs\.directories\.add\(generatedJniLibsDir\.get\(\)\.asFile\.path\)/u);
  assert.match(gradleIntegration, /dependsOn\(buildSonaUniffiAndroidLibraries\)/u);
  assert.match(androidReadme, /buildSonaUniffiAndroidLibraries/u);
  assert.match(androidReadme, /SONA_ANDROID_ABIS/u);
});

test('UniFFI Android Gradle integration tracks build inputs for incremental reruns', () => {
  const gradleIntegration = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sona-uniffi-bindings.gradle.kts'),
    'utf8',
  );

  for (const inputPath of [
    'Cargo.toml',
    'Cargo.lock',
    'adapters/local_asr/Cargo.toml',
    'adapters/local_asr/build.rs',
    'adapters/local_asr/src',
    'scripts/generate-uniffi-kotlin.js',
    'scripts/build-uniffi-android-libs.js',
    'scripts/android-sherpa-runtime.js',
    'platforms/android/packaging/sherpa-onnx-sources.json',
    'tools/uniffi_bindgen/Cargo.toml',
    'tools/uniffi_bindgen/src',
    'adapters/runtime_fs/Cargo.toml',
    'adapters/runtime_fs/src',
  ]) {
    assert.match(gradleIntegration, new RegExp(`inputs\\.(?:file|dir)\\(File\\(repoRoot, "${inputPath.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')}"\\)\\)`, 'u'));
  }
});

test('UniFFI Android Gradle integration avoids Gradle 10 deprecation warnings', () => {
  const gradleIntegration = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sona-uniffi-bindings.gradle.kts'),
    'utf8',
  );

  assert.doesNotMatch(gradleIntegration, /by tasks\.registering/u);
  assert.match(gradleIntegration, /val buildSonaUniffiAndroidLibraries = tasks\.register<Exec>\("buildSonaUniffiAndroidLibraries"\)/u);
  assert.match(gradleIntegration, /val generateSonaUniffiKotlin = tasks\.register<Exec>\("generateSonaUniffiKotlin"\)/u);
  assert.match(gradleIntegration, /KotlinCompile/u);
  assert.doesNotMatch(gradleIntegration, /println\(/u);
  assert.doesNotMatch(gradleIntegration, /\.srcDir\(/u);
  assert.match(gradleIntegration, /java\.directories\.add\(generatedKotlinDir\.get\(\)\.asFile\.path\)/u);
  assert.match(gradleIntegration, /jniLibs\.directories\.add\(generatedJniLibsDir\.get\(\)\.asFile\.path\)/u);
  assert.match(gradleIntegration, /tasks\.withType<KotlinCompile>\(\)/u);
  assert.match(gradleIntegration, /source\(generatedKotlinDir\)/u);
  assert.match(gradleIntegration, /it\.name\.startsWith\("extract"\) && it\.name\.endsWith\("Annotations"\)/u);
  assert.match(gradleIntegration, /dependsOn\(generateSonaUniffiKotlin\)/u);
});

test('UniFFI Android sample consumer imports generated Kotlin bindings', () => {
  const packageJson = JSON.parse(read('package.json'));
  const verifyScript = read('scripts', 'verify-uniffi-android-sample.js');
  const sampleSettings = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'settings.gradle.kts'),
    'utf8',
  );
  const sampleRootGradle = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'build.gradle.kts'),
    'utf8',
  );
  const sampleProperties = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'gradle.properties'),
    'utf8',
  );
  const sampleGradle = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'sample-library', 'build.gradle.kts'),
    'utf8',
  );
  const sampleKotlin = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin', 'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt'),
    'utf8',
  );

  assert.equal(packageJson.scripts['verify:android-uniffi'], 'node scripts/verify-uniffi-android-sample.js');
  assert.match(verifyScript, /generate-uniffi-kotlin\.js/u);
  assert.match(verifyScript, /build-uniffi-android-libs\.js/u);
  assert.match(verifyScript, /sample-consumer/u);
  assert.match(verifyScript, /--require-gradle/u);
  assert.match(verifyScript, /SONA_ANDROID_ABIS/u);
  assert.match(verifyScript, /gradle\.bat|gradle/u);
  assert.match(verifyScript, /:sample-library:assembleDebug/u);
  assert.doesNotMatch(verifyScript, /:sample-library:tasks/u);

  assert.match(sampleSettings, /pluginManagement/u);
  assert.match(sampleSettings, /id\("com\.android\.library"\) version "9\.2\.1" apply false/u);
  assert.match(sampleSettings, /include\(":sample-library"\)/u);
  assert.match(sampleRootGradle, /tasks\.register\("clean", Delete::class\)/u);
  assert.match(sampleProperties, /^SONA_REPO_ROOT=\.\.\/\.\.\/\.\.\/\.\.$/mu);
  assert.match(sampleGradle, /id\("com\.android\.library"\)/u);
  assert.doesNotMatch(sampleGradle, /org\.jetbrains\.kotlin\.android|kotlin-android/u);
  assert.match(sampleGradle, /namespace = "com\.sona\.uniffi\.sample"/u);
  assert.match(sampleGradle, /compileOptions\s*\{/u);
  assert.match(sampleGradle, /compilerOptions/u);
  assert.match(sampleGradle, /apply\(from = "\.\.\/\.\.\/sona-uniffi-bindings\.gradle\.kts"\)/u);

  for (const generatedImport of [
    'FfiLlmPromptChunk',
    'FfiPolishedSegment',
    'SonaCoreBindingException',
    'defaultConfigJson',
    'parsePolishChunkJson',
    'planPolishPromptChunksJson',
  ]) {
    assert.match(sampleKotlin, new RegExp(`import uniffi\\.sona_uniffi_bind\\.${generatedImport}`, 'u'));
  }

  assert.match(sampleKotlin, /object SonaUniffiSmoke/u);
  assert.match(sampleKotlin, /planPolishPromptChunksJson\(/u);
  assert.match(sampleKotlin, /parsePolishChunkJson\(/u);
});

test('UniFFI binding errors avoid Kotlin Throwable message conflicts', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');

  assert.match(uniffiLib, /InvalidInput\s*\{\s*reason: String\s*\}/u);
  assert.doesNotMatch(uniffiLib, /InvalidInput\s*\{\s*message: String\s*\}/u);
});

test('UniFFI Android Gradle smoke verifies assembled AAR contents', () => {
  const verifyScript = read('scripts', 'verify-uniffi-android-sample.js');
  const androidReadme = read('platforms', 'android', 'README.md');

  assert.match(verifyScript, /function readZipEntries/u);
  assert.match(verifyScript, /function verifyAndroidSampleAar/u);
  assert.match(verifyScript, /sample-library-debug\.aar/u);
  assert.match(verifyScript, /jni\/arm64-v8a\/libsona_uniffi_bind\.so/u);
  assert.match(verifyScript, /libsherpa-onnx-c-api\.so/u);
  assert.match(verifyScript, /libonnxruntime\.so/u);
  assert.match(verifyScript, /classes\.jar/u);
  assert.match(verifyScript, /uniffi\/sona_uniffi_bind\//u);
  assert.match(verifyScript, /uniffi\/sona_uniffi_bind\/FfiAsrStreamingSession/u);
  assert.match(verifyScript, /uniffi\/sona_uniffi_bind\/FfiAsrStreamingObserver/u);
  assert.match(verifyScript, /com\/sona\/uniffi\/sample\/SonaUniffiSmoke/u);
  assert.match(verifyScript, /com\/sona\/uniffi\/consumer\/SonaUniffiConsumerSmoke/u);
  assert.match(androidReadme, /assembles the sample debug AAR/u);
  assert.match(androidReadme, /jni\/arm64-v8a\/libsona_uniffi_bind\.so/u);
  assert.match(androidReadme, /libsherpa-onnx-c-api\.so/u);
  assert.match(androidReadme, /libonnxruntime\.so/u);
  assert.match(androidReadme, /object\s*:\s*FfiAsrStreamingObserver/u);
  assert.match(androidReadme, /createOnlineAsrStreamingSession/u);
  assert.match(androidReadme, /verify:android-uniffi:gradle/u);
});

test('UniFFI Android sample ignores generated Gradle outputs', () => {
  const androidIgnorePath = path.join(repoRoot, 'platforms', 'android', '.gitignore');

  assert.equal(fs.existsSync(androidIgnorePath), true);
  const androidIgnore = fs.readFileSync(androidIgnorePath, 'utf8');
  const ignoreRules = androidIgnore.split(/\r?\n/u);

  for (const ignoreRule of [
    '/generated/',
    '/sample-consumer/.gradle/',
    '/sample-consumer/build/',
    '/sample-consumer/**/build/',
  ]) {
    assert.ok(ignoreRules.includes(ignoreRule), `${androidIgnorePath} must include ${ignoreRule}`);
  }
});

test('UniFFI Android sample can use a repo-managed Gradle distribution', () => {
  const packageJson = JSON.parse(read('package.json'));
  const verifyScript = read('scripts', 'verify-uniffi-android-sample.js');
  const androidReadme = read('platforms', 'android', 'README.md');
  const gradleRunnerPath = path.join(repoRoot, 'scripts', 'run-managed-gradle.js');

  assert.equal(
    packageJson.scripts['verify:android-uniffi:gradle'],
    'node scripts/verify-uniffi-android-sample.js --download-gradle --require-gradle',
  );
  assert.match(verifyScript, /--download-gradle/u);
  assert.match(verifyScript, /run-managed-gradle\.js/u);
  assert.ok(fs.existsSync(gradleRunnerPath), 'scripts/run-managed-gradle.js should exist');

  const gradleRunner = fs.readFileSync(gradleRunnerPath, 'utf8');
  assert.match(gradleRunner, /DEFAULT_GRADLE_VERSION\s*=\s*'9\.6\.1'/u);
  assert.match(gradleRunner, /MAX_DOWNLOAD_ATTEMPTS\s*=\s*3/u);
  assert.match(gradleRunner, /9c0f7faeeb306cb14e4279a3e084ca6b596894089a0638e68a07c945a32c9e14/u);
  assert.match(gradleRunner, /https:\/\/services\.gradle\.org\/distributions\/gradle-\$\{gradleVersion\}-bin\.zip/u);
  assert.match(gradleRunner, /https:\/\/downloads\.gradle\.org\/distributions\/gradle-\$\{gradleVersion\}-bin\.zip/u);
  assert.match(gradleRunner, /--distribution-zip/u);
  assert.match(gradleRunner, /SONA_GRADLE_DISTRIBUTION_ZIP/u);
  assert.match(gradleRunner, /fs\.copyFileSync\(distributionZip/u);
  assert.match(gradleRunner, /function curlDownloadFile/u);
  assert.match(gradleRunner, /--location/u);
  assert.match(gradleRunner, /--retry/u);
  assert.match(gradleRunner, /attempt < MAX_DOWNLOAD_ATTEMPTS/u);
  assert.match(gradleRunner, /target[u]?['"], ['"]managed-gradle/u);
  assert.match(gradleRunner, /Expand-Archive/u);
  assert.match(gradleRunner, /spawnSync\(gradleExecutable/u);
  assert.match(gradleRunner, /shell:\s*process\.platform === 'win32'/u);
  assert.match(gradleRunner, /result\.error/u);
  assert.match(androidReadme, /verify:android-uniffi:gradle/u);
  assert.match(androidReadme, /SONA_GRADLE_DISTRIBUTION_ZIP/u);
  assert.match(androidReadme, /target\/managed-gradle/u);
});

test('UniFFI Android native build script supports a no-toolchain dry run', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-uniffi-android-dry-run-'));
  const targetDir = path.join(fixtureRoot, 'target');
  const outDir = path.join(fixtureRoot, 'jni-output');
  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'build-uniffi-android-libs.js'),
      '--dry-run',
      '--abis',
      'arm64-v8a',
      '--target-dir',
      targetDir,
      '--out-dir',
      outDir,
    ],
    {
      cwd: repoRoot,
      encoding: 'utf8',
      env: {
        ...process.env,
        SONA_SHERPA_ONNX_ANDROID_ARCHIVE: path.join(fixtureRoot, 'missing-archive.tar.bz2'),
      },
    },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(
    result.stdout,
    /Plan: arm64-v8a -> aarch64-linux-android; AAR native entries: jni\/arm64-v8a\/libsona_uniffi_bind\.so, jni\/arm64-v8a\/libsherpa-onnx-c-api\.so, jni\/arm64-v8a\/libonnxruntime\.so/u,
  );
  assert.doesNotMatch(result.stdout, /cargo build/u);
  assert.equal(fs.existsSync(path.join(targetDir, 'android-sherpa')), false);
  assert.equal(fs.existsSync(outDir), false);
});

test('UniFFI Android native build script rejects an empty ABI selection', () => {
  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'build-uniffi-android-libs.js'),
      '--dry-run',
      '--abis',
      ' , ',
      '--out-dir',
      path.join(os.tmpdir(), 'sona-uniffi-android-empty-abis'),
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );
  const output = `${result.stderr}\n${result.stdout}`;

  assert.notEqual(result.status, 0, output);
  assert.match(output, /At least one Android ABI is required/u);
});

test('UniFFI Android native build script skips incomplete auto-discovered NDK installs', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-android-ndk-'));
  const sdkRoot = path.join(tempRoot, 'sdk');
  const validNdk = path.join(sdkRoot, 'ndk', '29.0.14206865');
  const incompleteNdk = path.join(sdkRoot, 'ndk', '30.0.15729638');

  for (const abiCase of androidNdkAbiCases) {
    const validPaths = androidNdkToolPaths(validNdk, abiCase, process.platform);
    const incompletePaths = androidNdkToolPaths(incompleteNdk, abiCase, process.platform);
    fs.mkdirSync(path.dirname(validPaths.linkerPath), { recursive: true });
    fs.mkdirSync(path.dirname(incompletePaths.linkerPath), { recursive: true });
    fs.writeFileSync(validPaths.linkerPath, '');
    fs.writeFileSync(incompletePaths.linkerPath, '');
  }
  const validArchiverPath = androidNdkToolPaths(validNdk, androidNdkAbiCases[0], process.platform).archiverPath;
  fs.writeFileSync(validArchiverPath, '');

  for (const abiCase of androidNdkAbiCases) {
    const result = runAndroidNdkPrint({ abi: abiCase.abi, androidHome: sdkRoot });
    const validPaths = androidNdkToolPaths(validNdk, abiCase, process.platform);
    const targetEnvSuffix = abiCase.target.replace(/-/gu, '_');
    const expectedLines = [
      `CARGO_TARGET_${targetEnvSuffix.toUpperCase()}_LINKER=${validPaths.linkerPath}`,
      `CC_${targetEnvSuffix}=${validPaths.linkerPath}`,
      `AR_${targetEnvSuffix}=${validPaths.archiverPath}`,
    ];

    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.deepEqual(result.stdout.trim().split(/\r?\n/u), expectedLines);
    assert.doesNotMatch(result.stdout, /30\.0\.15729638/u);
  }
});

test('UniFFI Android native build script reports a missing linker in an explicit NDK', () => {
  const ndkHome = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-android-explicit-ndk-'));
  const abiCase = androidNdkAbiCases[0];
  const toolPaths = androidNdkToolPaths(ndkHome, abiCase, process.platform);
  fs.mkdirSync(path.dirname(toolPaths.archiverPath), { recursive: true });
  fs.writeFileSync(toolPaths.archiverPath, '');

  const result = runAndroidNdkPrint({ abi: abiCase.abi, ndkHome });
  const output = `${result.stderr}\n${result.stdout}`;

  assert.notEqual(result.status, 0, output);
  assert.ok(output.includes(`Missing Android NDK linker at ${toolPaths.linkerPath}`), output);
});

test('UniFFI Android native build script reports a missing archiver in an explicit NDK', () => {
  const ndkHome = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-android-explicit-ndk-'));
  const abiCase = androidNdkAbiCases[0];
  const toolPaths = androidNdkToolPaths(ndkHome, abiCase, process.platform);
  fs.mkdirSync(path.dirname(toolPaths.linkerPath), { recursive: true });
  fs.writeFileSync(toolPaths.linkerPath, '');

  const result = runAndroidNdkPrint({ abi: abiCase.abi, ndkHome });
  const output = `${result.stderr}\n${result.stdout}`;

  assert.notEqual(result.status, 0, output);
  assert.ok(output.includes(`Missing Android NDK archiver at ${toolPaths.archiverPath}`), output);
});

test('UniFFI Android native build script supports injected host toolchain layouts in print mode', () => {
  const abiCase = androidNdkAbiCases[0];

  for (const hostPlatform of ['windows', 'linux', 'darwin']) {
    const ndkHome = fs.mkdtempSync(path.join(os.tmpdir(), `sona-android-${hostPlatform}-ndk-`));
    const targetDir = path.join(ndkHome, 'target');
    const outDir = path.join(ndkHome, 'jni-output');
    const toolPaths = androidNdkToolPaths(ndkHome, abiCase, hostPlatform);
    fs.mkdirSync(path.dirname(toolPaths.linkerPath), { recursive: true });
    fs.writeFileSync(toolPaths.linkerPath, '');
    fs.writeFileSync(toolPaths.archiverPath, '');

    const result = runAndroidNdkPrint({
      abi: abiCase.abi,
      ndkHome,
      hostPlatform,
      targetDir,
      outDir,
      archiveOverride: path.join(ndkHome, 'missing-archive.tar.bz2'),
    });
    const targetEnvSuffix = abiCase.target.replace(/-/gu, '_');

    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.deepEqual(result.stdout.trim().split(/\r?\n/u), [
      `CARGO_TARGET_${targetEnvSuffix.toUpperCase()}_LINKER=${toolPaths.linkerPath}`,
      `CC_${targetEnvSuffix}=${toolPaths.linkerPath}`,
      `AR_${targetEnvSuffix}=${toolPaths.archiverPath}`,
    ]);
    assert.equal(
      result.stderr.trim(),
      `Plan: ${abiCase.abi} -> ${abiCase.target}; AAR native entries: ${[
        `jni/${abiCase.abi}/libsona_uniffi_bind.so`,
        `jni/${abiCase.abi}/libsherpa-onnx-c-api.so`,
        `jni/${abiCase.abi}/libonnxruntime.so`,
      ].join(', ')}`,
    );
    assert.equal(fs.existsSync(path.join(targetDir, 'android-sherpa')), false);
    assert.equal(fs.existsSync(outDir), false);
  }
});

test('UniFFI Android native build script rejects host overrides outside print mode', () => {
  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'build-uniffi-android-libs.js'),
      '--dry-run',
      '--host-platform',
      'linux',
      '--abis',
      'arm64-v8a',
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );
  const output = `${result.stderr}\n${result.stdout}`;

  assert.notEqual(result.status, 0, output);
  assert.match(output, /--host-platform is only supported with --print-linker-env/u);
});
