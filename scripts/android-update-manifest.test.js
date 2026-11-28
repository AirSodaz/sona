import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  createAndroidUpdateManifest,
  runCli,
  validateReleaseTagVersion,
} from './android-update-manifest.js';

const testVersion = '1.2.3';
const testNightlyVersion = `${testVersion}-123`;

test('Android update manifests preserve stable and nightly build identity', () => {
  assert.deepEqual(createAndroidUpdateManifest({
    channel: 'stable',
    versionName: testVersion,
    versionCode: '42',
  }), {
    schemaVersion: 1,
    channel: 'stable',
    versionName: testVersion,
    versionCode: 42,
  });
  assert.deepEqual(createAndroidUpdateManifest({
    channel: ' nightly ',
    versionName: ` ${testNightlyVersion} `,
    versionCode: 123,
  }), {
    schemaVersion: 1,
    channel: 'nightly',
    versionName: testNightlyVersion,
    versionCode: 123,
  });
});

test('Android update manifests reject invalid fields', () => {
  assert.throws(
    () => createAndroidUpdateManifest({ channel: 'preview', versionName: testVersion, versionCode: 1 }),
    /channel must be stable or nightly/u,
  );
  assert.throws(
    () => createAndroidUpdateManifest({ channel: 'stable', versionName: '', versionCode: 1 }),
    /versionName must contain/u,
  );
  for (const versionCode of ['0', '1.5', '2100000001']) {
    assert.throws(
      () => createAndroidUpdateManifest({ channel: 'stable', versionName: testVersion, versionCode }),
      /versionCode must be an integer/u,
    );
  }
});

test('release tags must exactly match the project version', () => {
  assert.equal(validateReleaseTagVersion(`v${testVersion}`, testVersion), testVersion);
  assert.throws(
    () => validateReleaseTagVersion('v0.7.5', testVersion),
    new RegExp(`must equal v${testVersion.replaceAll('.', '\\.')}`, 'u'),
  );
});

test('generate command writes the canonical schema', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-android-update-'));
  const outputPath = path.join(fixtureRoot, 'android-update.json');
  try {
    runCli([
      'generate',
      '--channel', 'stable',
      '--version-name', testVersion,
      '--version-code', '77',
      '--output', outputPath,
    ]);
    assert.deepEqual(JSON.parse(fs.readFileSync(outputPath, 'utf8')), {
      schemaVersion: 1,
      channel: 'stable',
      versionName: testVersion,
      versionCode: 77,
    });
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('generate command attaches ABI APK assets, sha256, size, and release metadata', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-android-update-assets-'));
  const artifactsDir = path.join(fixtureRoot, 'artifacts');
  const armDir = path.join(artifactsDir, 'arm64');
  const x64Dir = path.join(artifactsDir, 'x86_64');
  const outputPath = path.join(fixtureRoot, 'android-update.json');
  const notesPath = path.join(fixtureRoot, 'notes.txt');
  fs.mkdirSync(armDir, { recursive: true });
  fs.mkdirSync(x64Dir, { recursive: true });
  const armBytes = Buffer.from('arm64-apk-payload');
  const x64Bytes = Buffer.from('x86_64-apk-payload');
  fs.writeFileSync(path.join(armDir, 'app-arm64-v8a-debug.apk'), armBytes);
  fs.writeFileSync(path.join(x64Dir, 'app-x86_64-debug.apk'), x64Bytes);
  fs.writeFileSync(notesPath, 'Bug fixes and improvements\n', 'utf8');

  try {
    runCli([
      'generate',
      '--channel', 'stable',
      '--version-name', testVersion,
      '--version-code', '88',
      '--artifacts-dir', artifactsDir,
      '--download-base', 'https://github.com/AirSodaz/sona/releases/download/v1.2.3/',
      '--signing', 'debug',
      '--pub-date', '2026-11-28T12:00:00Z',
      '--notes-file', notesPath,
      '--output', outputPath,
    ]);

    const manifest = JSON.parse(fs.readFileSync(outputPath, 'utf8'));
    assert.deepEqual(manifest, {
      schemaVersion: 1,
      channel: 'stable',
      versionName: testVersion,
      versionCode: 88,
      pubDate: '2026-11-28T12:00:00Z',
      notes: 'Bug fixes and improvements',
      signing: 'debug',
      assets: {
        'arm64-v8a': {
          url: 'https://github.com/AirSodaz/sona/releases/download/v1.2.3/app-arm64-v8a-debug.apk',
          sha256: crypto.createHash('sha256').update(armBytes).digest('hex'),
          sizeBytes: armBytes.length,
        },
        'x86_64': {
          url: 'https://github.com/AirSodaz/sona/releases/download/v1.2.3/app-x86_64-debug.apk',
          sha256: crypto.createHash('sha256').update(x64Bytes).digest('hex'),
          sizeBytes: x64Bytes.length,
        },
      },
    });
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('Android update manifest rejects invalid asset fields', () => {
  assert.throws(
    () => createAndroidUpdateManifest({
      channel: 'stable',
      versionName: testVersion,
      versionCode: 1,
      assets: {
        'arm64-v8a': {
          url: 'http://insecure.example.com/app.apk',
          sha256: '0'.repeat(64),
          sizeBytes: 1024,
        },
      },
    }),
    /must start with https:\/\//u,
  );
  assert.throws(
    () => createAndroidUpdateManifest({
      channel: 'stable',
      versionName: testVersion,
      versionCode: 1,
      assets: {
        'arm64-v8a': {
          url: 'https://example.com/app.apk',
          sha256: 'not-a-hash',
          sizeBytes: 1024,
        },
      },
    }),
    /64 hex characters/u,
  );
});
