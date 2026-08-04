import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function read(...segments) {
  return fs.readFileSync(path.join(repoRoot, ...segments), 'utf8');
}

test('release version metadata stays aligned with the root package version', () => {
  const projectVersion = JSON.parse(read('package.json')).version;
  const versionPattern = projectVersion.replaceAll('.', '\\.');

  assert.match(projectVersion, /^\d+\.\d+\.\d+$/u);
  assert.equal(
    JSON.parse(read('platforms', 'desktop', 'frontend', 'package.json')).version,
    projectVersion,
  );
  assert.equal(
    JSON.parse(read('platforms', 'desktop', 'tauri.conf.json')).version,
    projectVersion,
  );
  assert.match(
    read('Cargo.toml'),
    new RegExp(`\\[workspace\\.package\\]\\s+version = "${versionPattern}"`, 'u'),
  );
  assert.match(
    read('platforms', 'android', 'client', 'app', 'build.gradle.kts'),
    new RegExp(`suppliedAndroidVersionName\\.ifEmpty \\{ "${versionPattern}" \\}`, 'u'),
  );
  assert.match(
    read('platforms', 'android', 'sample-consumer', 'sample-library', 'build.gradle.kts'),
    new RegExp(`^version = "${versionPattern}"$`, 'mu'),
  );
  assert.match(
    read('platforms', 'android', 'sample-consumer', 'consumer-library', 'build.gradle.kts'),
    new RegExp(`com\\.sona:sona-uniffi-bindings:${versionPattern}`, 'u'),
  );

  const androidReadme = read('platforms', 'android', 'README.md');
  assert.equal(
    [...androidReadme.matchAll(new RegExp(`com\\.sona:sona-uniffi-bindings:${versionPattern}`, 'gu'))]
      .length,
    2,
  );
});
