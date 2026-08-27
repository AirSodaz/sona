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

test('Android Gradle dependencies stay on the reviewed versions', () => {
  const appGradle = read('platforms', 'android', 'client', 'app', 'build.gradle.kts');
  const androidAdapterGradle = read(
    'platforms', 'android', 'client', 'adapters', 'android', 'build.gradle.kts',
  );
  const bindingsGradle = read('platforms', 'android', 'sona-uniffi-bindings.gradle.kts');

  assert.match(appGradle, /lifecycle-process:2\.11\.0/u);
  assert.match(appGradle, /kotlinx-coroutines-test:1\.11\.0/u);
  assert.match(appGradle, /work-runtime-ktx:2\.11\.2/u);
  assert.match(androidAdapterGradle, /work-runtime-ktx:2\.11\.2/u);
  assert.match(
    bindingsGradle,
    /net\.java\.dev\.jna:jna:5\.19\.1@aar/u,
    'Android JNA must use the API 37-verified 16 KB-aligned release',
  );
  assert.match(
    appGradle,
    /coreLibraryDesugaring\("com\.android\.tools:desugar_jdk_libs:2\.1\.5"\)/u,
  );
});
