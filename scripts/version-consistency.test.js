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
test('CUDA addon dependency versions stay aligned across workflow and scripts', () => {
  const cargoToml = read('Cargo.toml');
  const desktopCi = read('.github', 'workflows', 'desktop-client.yml');
  const cudaAddonCi = read('.github', 'workflows', 'cuda-addon.yml');
  const setupSherpaAction = read('.github', 'actions', 'setup-sherpa-onnx', 'action.yml');
  const buildCudaAddonScript = read('scripts', 'build-cuda-addon.js');

  const cargoSherpaVersion = cargoToml.match(/sherpa-onnx\s*=\s*\{\s*version\s*=\s*"=([\d.]+)"/u)?.[1];
  assert.ok(cargoSherpaVersion, 'Cargo.toml must declare sherpa-onnx version');

  const desktopSherpaVersion = desktopCi.match(/SHERPA_ONNX_VERSION:\s*([\d.]+)/u)?.[1];
  assert.equal(desktopSherpaVersion, cargoSherpaVersion, 'desktop CI sherpa-onnx version must match Cargo.toml');

  const cudaAddonSherpaVersion = cudaAddonCi.match(/sherpa_onnx_version:[\s\S]*?default:\s*"([\d.]+)"/u)?.[1];
  assert.equal(cudaAddonSherpaVersion, cargoSherpaVersion, 'cuda-addon CI sherpa-onnx version must match Cargo.toml');

  const setupSherpaDefaultVersion = setupSherpaAction.match(/version:[\s\S]*?default:\s*'([\d.]+)'/u)?.[1];
  assert.equal(setupSherpaDefaultVersion, cargoSherpaVersion, 'setup-sherpa-onnx action default version must match Cargo.toml');

  const scriptSherpaVersion = buildCudaAddonScript.match(/sherpaVersion\s*=\s*options\['sherpa-version'\]\s*\|\|\s*'([\d.]+)'/u)?.[1];
  assert.equal(scriptSherpaVersion, cargoSherpaVersion, 'build-cuda-addon.js default sherpaVersion must match Cargo.toml');

  const desktopLlamaRef = desktopCi.match(/LLAMA_CPP_REF:\s*([a-zA-Z0-9_-]+)/u)?.[1];
  assert.ok(desktopLlamaRef, 'desktop CI must declare LLAMA_CPP_REF');

  const cudaAddonLlamaRef = cudaAddonCi.match(/llama_cpp_ref:[\s\S]*?default:\s*"([a-zA-Z0-9_-]+)"/u)?.[1];
  assert.equal(cudaAddonLlamaRef, desktopLlamaRef, 'cuda-addon CI llama_cpp_ref must match desktop CI');

  const scriptLlamaRef = buildCudaAddonScript.match(/llamaRef\s*=\s*options\['llama-ref'\]\s*\|\|\s*'([a-zA-Z0-9_-]+)'/u)?.[1];
  assert.equal(scriptLlamaRef, desktopLlamaRef, 'build-cuda-addon.js default llamaRef must match desktop CI');

  const cudaAddonOrtVersion = cudaAddonCi.match(/onnxruntime_version:[\s\S]*?default:\s*"([\d.]+)"/u)?.[1];
  assert.ok(cudaAddonOrtVersion, 'cuda-addon CI must declare default onnxruntime_version');

  const scriptOrtVersion = buildCudaAddonScript.match(/ortVersion\s*=\s*options\['ort-version'\]\s*\|\|\s*'([\d.]+)'/u)?.[1];
  assert.equal(scriptOrtVersion, cudaAddonOrtVersion, 'build-cuda-addon.js default ortVersion must match cuda-addon CI');
  const rustAddonVersion = read('adapters', 'model_downloads', 'src', 'cuda_addon.rs')
    .match(/DEFAULT_CUDA_ADDON_VERSION:\s*&str\s*=\s*"([\d.]+)"/u)?.[1];
  assert.ok(rustAddonVersion, 'adapters/model_downloads must declare DEFAULT_CUDA_ADDON_VERSION');

  const cudaAddonDefaultVersion = cudaAddonCi.match(/addon_version:[\s\S]*?default:\s*"([\d.]+)"/u)?.[1];
  assert.equal(cudaAddonDefaultVersion, rustAddonVersion, 'cuda-addon CI default version must match Rust DEFAULT_CUDA_ADDON_VERSION');

  const scriptAddonVersion = buildCudaAddonScript.match(/addonVersion\s*=\s*options\['addon-version'\]\s*\|\|\s*'([\d.]+)'/u)?.[1];
  assert.equal(scriptAddonVersion, rustAddonVersion, 'build-cuda-addon.js default addonVersion must match Rust DEFAULT_CUDA_ADDON_VERSION');
});
