import assert from 'node:assert/strict';
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
