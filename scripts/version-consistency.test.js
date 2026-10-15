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
    read('platforms', 'android', 'sample-consumer', 'gradle.properties'),
    new RegExp(`^SONA_VERSION=${versionPattern}$`, 'mu'),
  );
  assert.match(
    read('platforms', 'android', 'sample-consumer', 'sample-library', 'build.gradle.kts'),
    /version = sonaVersion/u,
  );
  assert.match(
    read('platforms', 'android', 'sample-consumer', 'consumer-library', 'build.gradle.kts'),
    /com\.sona:sona-uniffi-bindings:\$sonaVersion/u,
  );

  const androidReadme = read('platforms', 'android', 'README.md');
  assert.equal(
    [...androidReadme.matchAll(/com\.sona:sona-uniffi-bindings:<version>/gu)].length,
    2,
  );
});

test('Android Gradle dependencies stay aligned across modules and Cargo', () => {
  const appGradle = read('platforms', 'android', 'client', 'app', 'build.gradle.kts');
  const androidAdapterGradle = read(
    'platforms', 'android', 'client', 'adapters', 'android', 'build.gradle.kts',
  );
  const applicationGradle = read(
    'platforms', 'android', 'client', 'application', 'build.gradle.kts',
  );
  const uniffiAdapterGradle = read(
    'platforms', 'android', 'client', 'adapters', 'uniffi', 'build.gradle.kts',
  );
  const bindingsGradle = read('platforms', 'android', 'sona-uniffi-bindings.gradle.kts');
  const cargoLock = read('Cargo.lock');

  // WorkManager version alignment between app and adapters
  const appWorkVersion = appGradle.match(/work-runtime-ktx:([\d.]+)/u)?.[1];
  const adapterWorkVersion = androidAdapterGradle.match(/work-runtime-ktx:([\d.]+)/u)?.[1];
  const adapterWorkTestingVersion = androidAdapterGradle.match(/work-testing:([\d.]+)/u)?.[1];
  assert.ok(appWorkVersion, 'app must declare work-runtime-ktx');
  assert.equal(appWorkVersion, adapterWorkVersion, 'work-runtime-ktx version must match across app and adapters');
  assert.equal(appWorkVersion, adapterWorkTestingVersion, 'work-testing version must match work-runtime-ktx');

  // Coroutines version alignment across all Android modules
  const appCoroutinesTest = appGradle.match(/kotlinx-coroutines-test:([\d.]+)/u)?.[1];
  const appCoroutinesCore = applicationGradle.match(/kotlinx-coroutines-core:([\d.]+)/u)?.[1];
  const adapterCoroutinesCore = androidAdapterGradle.match(/kotlinx-coroutines-core:([\d.]+)/u)?.[1];
  const uniffiCoroutinesCore = uniffiAdapterGradle.match(/kotlinx-coroutines-core:([\d.]+)/u)?.[1];
  const bindingsCoroutinesCore = bindingsGradle.match(/kotlinx-coroutines-core:([\d.]+)/u)?.[1];
  assert.ok(appCoroutinesTest, 'app must declare kotlinx-coroutines-test');
  assert.equal(appCoroutinesCore, appCoroutinesTest, 'application coroutines core must match app coroutines test');
  assert.equal(adapterCoroutinesCore, appCoroutinesTest, 'adapter coroutines core must match app coroutines test');
  assert.equal(uniffiCoroutinesCore, appCoroutinesTest, 'uniffi coroutines core must match app coroutines test');
  assert.equal(bindingsCoroutinesCore, appCoroutinesTest, 'bindings coroutines core must match app coroutines test');

  // rustls-platform-verifier version alignment between Cargo.lock and Android app
  const cargoVerifierMatch = cargoLock.match(
    /name = "rustls-platform-verifier-android"\s+version = "([\d.]+)"/u,
  );
  const cargoVerifierVersion = cargoVerifierMatch?.[1];
  const androidVerifierVersion = appGradle.match(/rustls-platform-verifier:([\d.]+)/u)?.[1];
  assert.ok(cargoVerifierVersion, 'Cargo.lock must include rustls-platform-verifier-android');
  assert.equal(
    androidVerifierVersion,
    cargoVerifierVersion,
    'Android app rustls-platform-verifier must match Cargo.lock rustls-platform-verifier-android',
  );
});
