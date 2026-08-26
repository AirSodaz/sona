import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { repoRoot } from './test-support/repo-root.js';
import {
  makeTempRepo,
  node,
  runtimeLibraryNames,
  writeCanonicalAppBundle,
  writeGeneratedBundleConfig,
  writeRuntimeLibraries,
} from './test-support/desktop-packaging-fixtures.js';

test('tauri bundle verification inspects canonical native app locations', () => {
  for (const target of [
    'x86_64-pc-windows-msvc',
    'aarch64-apple-darwin',
    'x86_64-unknown-linux-gnu',
  ]) {
    const root = makeTempRepo();
    const stagingRoot = path.join(root, 'target', 'desktop-bundle', target);
    const sidecarsDir = path.join(stagingRoot, 'sidecars');
    const runtimeLibDir = path.join(stagingRoot, 'runtime-libs');
    const configPath = path.join(stagingRoot, 'tauri.bundle.conf.json');
    fs.mkdirSync(sidecarsDir, { recursive: true });
    fs.mkdirSync(runtimeLibDir, { recursive: true });
    fs.writeFileSync(path.join(sidecarsDir, `ffmpeg-${target}${target.includes('windows') ? '.exe' : ''}`), 'ffmpeg');
    fs.writeFileSync(path.join(sidecarsDir, `sona-cli-${target}${target.includes('windows') ? '.exe' : ''}`), 'cli');
    writeRuntimeLibraries(runtimeLibDir, target);
    writeGeneratedBundleConfig(configPath, target, sidecarsDir, runtimeLibDir);
    writeCanonicalAppBundle(root, target);

    const result = spawnSync(
      node,
      [
        path.join(repoRoot, 'platforms', 'desktop', 'scripts', 'verify-tauri-bundle.js'),
        '--repo-root',
        root,
        '--target',
        target,
        '--config',
        configPath,
      ],
      { cwd: repoRoot, encoding: 'utf8' },
    );

    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /Verified canonical app bundle/u);
  }
});

test('tauri bundle verification accepts an explicit native Windows bundle root', () => {
  const target = 'x86_64-pc-windows-msvc';
  const root = makeTempRepo();
  const releaseDir = path.join(root, 'target', 'release');
  const stagingRoot = path.join(root, 'target', 'desktop-bundle', target);
  const sidecarsDir = path.join(stagingRoot, 'sidecars');
  const runtimeLibDir = path.join(stagingRoot, 'runtime-libs');
  const configPath = path.join(stagingRoot, 'tauri.bundle.conf.json');
  fs.mkdirSync(sidecarsDir, { recursive: true });
  fs.mkdirSync(runtimeLibDir, { recursive: true });
  fs.writeFileSync(path.join(sidecarsDir, `ffmpeg-${target}.exe`), 'ffmpeg');
  fs.writeFileSync(path.join(sidecarsDir, `sona-cli-${target}.exe`), 'cli');
  writeRuntimeLibraries(runtimeLibDir, target);
  writeGeneratedBundleConfig(configPath, target, sidecarsDir, runtimeLibDir);
  writeCanonicalAppBundle(root, target, releaseDir);

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'platforms', 'desktop', 'scripts', 'verify-tauri-bundle.js'),
      '--repo-root',
      root,
      '--target',
      target,
      '--config',
      configPath,
      '--bundle-root',
      path.join(releaseDir, 'bundle'),
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Verified canonical app bundle/u);
});

test('tauri bundle verification omits llama.cpp libraries for static Linux builds', () => {
  const target = 'x86_64-unknown-linux-gnu';
  const root = makeTempRepo();
  const stagingRoot = path.join(root, 'target', 'desktop-bundle', target);
  const sidecarsDir = path.join(stagingRoot, 'sidecars');
  const runtimeLibDir = path.join(stagingRoot, 'runtime-libs');
  const configPath = path.join(stagingRoot, 'tauri.bundle.conf.json');
  fs.mkdirSync(sidecarsDir, { recursive: true });
  fs.mkdirSync(runtimeLibDir, { recursive: true });
  fs.writeFileSync(path.join(sidecarsDir, `ffmpeg-${target}`), 'ffmpeg');
  fs.writeFileSync(path.join(sidecarsDir, `sona-cli-${target}`), 'cli');
  writeRuntimeLibraries(runtimeLibDir, target);
  for (const name of runtimeLibraryNames(target).filter((value) => /^(?:lib)?(?:ggml|llama)/u.test(value))) {
    fs.rmSync(path.join(runtimeLibDir, name));
  }
  writeGeneratedBundleConfig(configPath, target, sidecarsDir, runtimeLibDir);
  writeCanonicalAppBundle(root, target);
  const canonicalRuntimeDir = path.join(
    root,
    'target',
    target,
    'release',
    'bundle',
    'appimage',
    'Sona.AppDir',
    'usr',
    'lib',
    'sona',
  );
  for (const name of runtimeLibraryNames(target).filter((value) => /^(?:lib)?(?:ggml|llama)/u.test(value))) {
    fs.rmSync(path.join(canonicalRuntimeDir, name));
  }

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'platforms', 'desktop', 'scripts', 'verify-tauri-bundle.js'),
      '--repo-root',
      root,
      '--target',
      target,
      '--config',
      configPath,
    ],
    {
      cwd: repoRoot,
      encoding: 'utf8',
      env: { ...process.env, LLAMA_BUILD_SHARED_LIBS: '0' },
    },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Verified packaged artifacts/u);
});

test('tauri bundle verification does not mix native and target-qualified Windows outputs', () => {
  const target = 'aarch64-pc-windows-msvc';
  const root = makeTempRepo();
  const stagingRoot = path.join(root, 'target', 'desktop-bundle', target);
  const sidecarsDir = path.join(stagingRoot, 'sidecars');
  const runtimeLibDir = path.join(stagingRoot, 'runtime-libs');
  const configPath = path.join(stagingRoot, 'tauri.bundle.conf.json');
  const targetReleaseDir = path.join(root, 'target', target, 'release');
  fs.mkdirSync(sidecarsDir, { recursive: true });
  fs.mkdirSync(runtimeLibDir, { recursive: true });
  fs.writeFileSync(path.join(sidecarsDir, `ffmpeg-${target}.exe`), 'ffmpeg');
  fs.writeFileSync(path.join(sidecarsDir, `sona-cli-${target}.exe`), 'cli');
  writeRuntimeLibraries(runtimeLibDir, target);
  writeGeneratedBundleConfig(configPath, target, sidecarsDir, runtimeLibDir);
  writeCanonicalAppBundle(root, target, path.join(root, 'target', 'release'));
  writeCanonicalAppBundle(root, target, targetReleaseDir);
  fs.rmSync(path.join(targetReleaseDir, 'bundle', 'nsis'), { recursive: true });

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'platforms', 'desktop', 'scripts', 'verify-tauri-bundle.js'),
      '--repo-root',
      root,
      '--target',
      target,
      '--config',
      configPath,
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.notEqual(result.status, 0, result.stdout);
  assert.match(result.stderr, /No installer artifact was found/u);
});

test('tauri bundle verification requires ggml-vulkan only for Vulkan-enabled Windows builds', () => {
  const target = 'x86_64-pc-windows-msvc';
  const root = makeTempRepo();
  const stagingRoot = path.join(root, 'target', 'desktop-bundle', target);
  const sidecarsDir = path.join(stagingRoot, 'sidecars');
  const runtimeLibDir = path.join(stagingRoot, 'runtime-libs');
  const configPath = path.join(stagingRoot, 'tauri.bundle.conf.json');
  fs.mkdirSync(sidecarsDir, { recursive: true });
  fs.mkdirSync(runtimeLibDir, { recursive: true });
  fs.writeFileSync(path.join(sidecarsDir, `ffmpeg-${target}.exe`), 'ffmpeg');
  fs.writeFileSync(path.join(sidecarsDir, `sona-cli-${target}.exe`), 'cli');
  writeRuntimeLibraries(runtimeLibDir, target);
  writeGeneratedBundleConfig(configPath, target, sidecarsDir, runtimeLibDir);
  writeCanonicalAppBundle(root, target);

  const runVerifier = (environment) => spawnSync(
    node,
    [
      path.join(repoRoot, 'platforms', 'desktop', 'scripts', 'verify-tauri-bundle.js'),
      '--repo-root',
      root,
      '--target',
      target,
      '--config',
      configPath,
    ],
    { cwd: repoRoot, encoding: 'utf8', env: environment },
  );

  // Default CPU-only build: ggml-vulkan.dll must not be required.
  const cpuResult = runVerifier({ ...process.env });
  assert.equal(cpuResult.status, 0, cpuResult.stderr || cpuResult.stdout);

  // Vulkan-enabled build with the library staged: verification passes.
  const canonicalRuntimeDir = path.join(root, 'target', target, 'release');
  fs.writeFileSync(path.join(canonicalRuntimeDir, 'ggml-vulkan.dll'), 'vulkan');
  fs.writeFileSync(path.join(runtimeLibDir, 'ggml-vulkan.dll'), 'vulkan');
  const vulkanResult = runVerifier({ ...process.env, LLAMA_ENABLE_VULKAN: '1' });
  assert.equal(vulkanResult.status, 0, vulkanResult.stderr || vulkanResult.stdout);
});
