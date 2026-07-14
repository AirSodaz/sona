import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { repoRoot } from './test-support/repository.js';
import {
  androidNdkAbiCases,
  androidNdkToolPaths,
  node,
  runAndroidNdkPrint,
} from './test-support/android-ndk-fixtures.js';

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
