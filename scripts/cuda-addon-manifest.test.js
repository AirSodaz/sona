import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { repoRoot } from './test-support/repo-root.js';
import {
  createCudaAddonManifest,
  validateAddonVersion,
  validateReleaseTag,
} from './cuda-addon-manifest.js';
test('validateAddonVersion accepts valid semver and rejects invalid inputs', () => {
  assert.equal(validateAddonVersion('0.1.0'), '0.1.0');
  assert.equal(validateAddonVersion('1.2.3-beta.1'), '1.2.3-beta.1');
  assert.throws(() => validateAddonVersion('v1.0.0'), /valid semantic version/u);
  assert.throws(() => validateAddonVersion(''), /valid semantic version/u);
  assert.throws(() => validateAddonVersion('1.0'), /valid semantic version/u);
});

test('validateReleaseTag validates tag format and version parity', () => {
  assert.equal(validateReleaseTag('cuda-addon-v0.1.0'), '0.1.0');
  assert.equal(validateReleaseTag('cuda-addon-v1.0.0', '1.0.0'), '1.0.0');
  assert.throws(() => validateReleaseTag('v0.1.0'), /CUDA addon release tag must match/u);
  assert.throws(() => validateReleaseTag('cuda-addon-v0.1.0', '0.2.0'), /does not match expected version/u);
});

test('createCudaAddonManifest generates schema with platform hashes and URLs', () => {
  const testBaseDir = path.join(repoRoot, 'target', 'test-temp');
  fs.mkdirSync(testBaseDir, { recursive: true });
  const tempDir = fs.mkdtempSync(path.join(testBaseDir, 'cuda-manifest-'));
  try {
    const winFile = path.join(tempDir, 'sona-cuda-addon-v0.1.0-windows-x64.tar.gz');
    const linuxFile = path.join(tempDir, 'sona-cuda-addon-v0.1.0-linux-x64.tar.gz');

    fs.writeFileSync(winFile, 'dummy windows tar content');
    fs.writeFileSync(linuxFile, 'dummy linux tar content');

    const manifest = createCudaAddonManifest({
      addonVersion: '0.1.0',
      cudaVersion: '12.4',
      repo: 'test-org/sona',
      artifactsDir: tempDir,
      publishedAt: '2026-08-28T12:00:00.000Z',
    });

    assert.equal(manifest.schemaVersion, 1);
    assert.equal(manifest.addonVersion, '0.1.0');
    assert.equal(manifest.cudaVersion, '12.4');
    assert.equal(manifest.publishedAt, '2026-08-28T12:00:00.000Z');

    assert.ok(manifest.platforms['windows-x86_64']);
    assert.equal(
      manifest.platforms['windows-x86_64'].url,
      'https://github.com/test-org/sona/releases/download/cuda-addon-v0.1.0/sona-cuda-addon-v0.1.0-windows-x64.tar.gz',
    );
    assert.equal(typeof manifest.platforms['windows-x86_64'].sha256, 'string');
    assert.equal(manifest.platforms['windows-x86_64'].sha256.length, 64);
    assert.equal(manifest.platforms['windows-x86_64'].sizeBytes, 25);

    assert.ok(manifest.platforms['linux-x86_64']);
    assert.equal(
      manifest.platforms['linux-x86_64'].url,
      'https://github.com/test-org/sona/releases/download/cuda-addon-v0.1.0/sona-cuda-addon-v0.1.0-linux-x64.tar.gz',
    );
    assert.equal(typeof manifest.platforms['linux-x86_64'].sha256, 'string');
    assert.equal(manifest.platforms['linux-x86_64'].sha256.length, 64);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});
