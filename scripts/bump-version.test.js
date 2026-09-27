import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  bumpVersionFiles,
  computeNextVersion,
  parseSemver,
  replaceInFile,
} from './bump-version.js';

test('parseSemver parses valid semantic versions and rejects invalid ones', () => {
  assert.deepEqual(parseSemver('1.2.3'), { major: 1, minor: 2, patch: 3, version: '1.2.3' });
  assert.deepEqual(parseSemver('v0.8.2'), { major: 0, minor: 8, patch: 2, version: '0.8.2' });
  assert.deepEqual(parseSemver('  v10.20.30  '), { major: 10, minor: 20, patch: 30, version: '10.20.30' });

  assert.throws(() => parseSemver(''), /Expected semantic version/u);
  assert.throws(() => parseSemver('1.2'), /Expected semantic version/u);
  assert.throws(() => parseSemver('1.2.3.4'), /Expected semantic version/u);
  assert.throws(() => parseSemver('1.2.x'), /Expected semantic version/u);
});

test('computeNextVersion calculates patch, minor, major increments and explicit versions', () => {
  assert.equal(computeNextVersion('0.8.1', 'patch'), '0.8.2');
  assert.equal(computeNextVersion('0.8.1', 'minor'), '0.9.0');
  assert.equal(computeNextVersion('0.8.1', 'major'), '1.0.0');
  assert.equal(computeNextVersion('0.8.1', '1.0.5'), '1.0.5');
  assert.equal(computeNextVersion('0.8.1', 'v2.0.0'), '2.0.0');
});

test('bumpVersionFiles updates all 6 configuration files accurately in a repository directory', () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-bump-test-'));

  try {
    // 1. Root package.json
    fs.writeFileSync(
      path.join(tempDir, 'package.json'),
      JSON.stringify({ name: 'sona', version: '0.8.1' }, null, 2),
      'utf8',
    );

    // 2. Cargo.toml
    fs.writeFileSync(
      path.join(tempDir, 'Cargo.toml'),
      '[workspace.package]\nversion = "0.8.1"\nauthors = ["AirSodaz"]\n',
      'utf8',
    );

    // 3. Desktop frontend package.json
    const desktopFrontendDir = path.join(tempDir, 'platforms', 'desktop', 'frontend');
    fs.mkdirSync(desktopFrontendDir, { recursive: true });
    fs.writeFileSync(
      path.join(desktopFrontendDir, 'package.json'),
      JSON.stringify({ name: 'sona-desktop-frontend', version: '0.8.1' }, null, 2),
      'utf8',
    );

    // 4. Desktop tauri.conf.json
    const desktopDir = path.join(tempDir, 'platforms', 'desktop');
    fs.writeFileSync(
      path.join(desktopDir, 'tauri.conf.json'),
      JSON.stringify({ productName: 'Sona', version: '0.8.1' }, null, 2),
      'utf8',
    );

    // 5. Android app build.gradle.kts
    const androidAppDir = path.join(tempDir, 'platforms', 'android', 'client', 'app');
    fs.mkdirSync(androidAppDir, { recursive: true });
    fs.writeFileSync(
      path.join(androidAppDir, 'build.gradle.kts'),
      'val sonaAndroidVersionName = suppliedAndroidVersionName.ifEmpty { "0.8.1" }\n',
      'utf8',
    );

    // 6. Android sample-consumer gradle.properties
    const sampleConsumerDir = path.join(tempDir, 'platforms', 'android', 'sample-consumer');
    fs.mkdirSync(sampleConsumerDir, { recursive: true });
    fs.writeFileSync(
      path.join(sampleConsumerDir, 'gradle.properties'),
      'SONA_REPO_ROOT=../../../..\nSONA_VERSION=0.8.1\n',
      'utf8',
    );

    // Execute bump
    const result = bumpVersionFiles('patch', { repoRoot: tempDir });
    assert.equal(result.previousVersion, '0.8.1');
    assert.equal(result.targetVersion, '0.8.2');

    // Verify root package.json
    assert.equal(
      JSON.parse(fs.readFileSync(path.join(tempDir, 'package.json'), 'utf8')).version,
      '0.8.2',
    );

    // Verify Cargo.toml
    assert.match(
      fs.readFileSync(path.join(tempDir, 'Cargo.toml'), 'utf8'),
      /\[workspace\.package\]\s+version = "0\.8\.2"/u,
    );

    // Verify frontend package.json
    assert.equal(
      JSON.parse(fs.readFileSync(path.join(desktopFrontendDir, 'package.json'), 'utf8')).version,
      '0.8.2',
    );

    // Verify tauri.conf.json
    assert.equal(
      JSON.parse(fs.readFileSync(path.join(desktopDir, 'tauri.conf.json'), 'utf8')).version,
      '0.8.2',
    );

    // Verify Android app gradle
    assert.match(
      fs.readFileSync(path.join(androidAppDir, 'build.gradle.kts'), 'utf8'),
      /suppliedAndroidVersionName\.ifEmpty \{ "0\.8\.2" \}/u,
    );

    // Verify Android sample-consumer gradle.properties
    assert.match(
      fs.readFileSync(path.join(sampleConsumerDir, 'gradle.properties'), 'utf8'),
      /^SONA_VERSION=0\.8\.2$/mu,
    );
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test('replaceInFile throws when target pattern does not exist in file', () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-bump-fail-'));
  try {
    const dummyFile = path.join(tempDir, 'dummy.txt');
    fs.writeFileSync(dummyFile, 'hello world', 'utf8');
    assert.throws(
      () => replaceInFile(dummyFile, /missing-pattern/u, 'replacement'),
      /Pattern \/missing-pattern\/u not found in/u,
    );
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});
