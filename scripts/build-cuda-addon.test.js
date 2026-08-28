import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { repoRoot } from './test-support/repo-root.js';
import {
  REQUIRED_FILES,
  normalizePlatform,
  resolveLocalCudaRuntimeDir,
  detectCudaEnvironment,
  collectCudaToolkitLibraries,
  downloadLlamaCppCuda,
  stageCudaAddonFiles,
  packageCudaAddonArchive,
} from './build-cuda-addon.js';

test('normalizePlatform handles win32 and linux correctly', () => {
  assert.equal(normalizePlatform('win32'), 'windows-x86_64');
  assert.equal(normalizePlatform('linux'), 'linux-x86_64');
  assert.equal(normalizePlatform('darwin'), 'darwin');
});

test('resolveLocalCudaRuntimeDir resolves platform-specific paths', () => {
  const mockWinAppData = path.join(repoRoot, 'target', 'mock-appdata');
  const winDir = resolveLocalCudaRuntimeDir('win32', { LOCALAPPDATA: mockWinAppData });
  assert.equal(winDir, path.join(mockWinAppData, 'com.asoda.sona', 'runtimes', 'cuda'));

  const mockLinuxData = path.join(repoRoot, 'target', 'mock-xdg-data');
  const linuxDir = resolveLocalCudaRuntimeDir('linux', { XDG_DATA_HOME: mockLinuxData });
  assert.equal(linuxDir, path.join(mockLinuxData, 'com.asoda.sona', 'runtimes', 'cuda'));

  assert.equal(resolveLocalCudaRuntimeDir('darwin'), null);
});

test('detectCudaEnvironment inspects mock environment without crashing', () => {
  const env = detectCudaEnvironment({ CUDA_PATH: '/mock/cuda' }, 'win32');
  assert.equal(env.platform, 'windows-x86_64');
  assert.equal(env.supported, true);
  assert.equal(env.cudaPath, '/mock/cuda');
});

test('collectCudaToolkitLibraries filters matching DLLs and SOs', () => {
  const testBaseDir = path.join(repoRoot, 'target', 'test-temp');
  fs.mkdirSync(testBaseDir, { recursive: true });
  const tempDir = fs.mkdtempSync(path.join(testBaseDir, 'cuda-collect-'));
  try {
    const binDir = path.join(tempDir, 'bin');
    fs.mkdirSync(binDir, { recursive: true });
    fs.writeFileSync(path.join(binDir, 'cudart64_12.dll'), '');
    fs.writeFileSync(path.join(binDir, 'cublas64_12.dll'), '');
    fs.writeFileSync(path.join(binDir, 'cublasLt64_12.dll'), '');
    fs.writeFileSync(path.join(binDir, 'unrelated.dll'), '');
    fs.writeFileSync(path.join(binDir, 'test.txt'), '');

    const collected = collectCudaToolkitLibraries(tempDir, 'win32');
    assert.equal(collected.length, 3);
    assert.ok(collected.some((f) => f.includes('cudart64_12.dll')));
    assert.ok(collected.some((f) => f.includes('cublas64_12.dll')));
    assert.ok(collected.some((f) => f.includes('cublasLt64_12.dll')));
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test('stageCudaAddonFiles copies all components to staged directory', () => {
  const testBaseDir = path.join(repoRoot, 'target', 'test-temp');
  fs.mkdirSync(testBaseDir, { recursive: true });
  const tempRoot = fs.mkdtempSync(path.join(testBaseDir, 'cuda-stage-'));
  try {
    const ortDir = path.join(tempRoot, 'ort');
    const llamaDir = path.join(tempRoot, 'llama');
    const stagedDir = path.join(tempRoot, 'staged');
    const extraCudaLib = path.join(tempRoot, 'cudart64_12.dll');

    fs.mkdirSync(ortDir, { recursive: true });
    fs.mkdirSync(llamaDir, { recursive: true });
    fs.writeFileSync(extraCudaLib, 'cuda runtime');

    fs.writeFileSync(path.join(ortDir, 'onnxruntime_providers_cuda.dll'), 'ort cuda');
    fs.writeFileSync(path.join(ortDir, 'onnxruntime_providers_shared.dll'), 'ort shared');
    fs.writeFileSync(path.join(llamaDir, 'ggml-cuda.dll'), 'llama cuda');

    const copied = stageCudaAddonFiles({
      stagedDir,
      ortLibDir: ortDir,
      llamaLibDir: llamaDir,
      cudaLibPaths: [extraCudaLib],
      platform: 'win32',
    });
    assert.equal(copied.length, 6);
    assert.ok(fs.existsSync(path.join(stagedDir, 'onnxruntime_providers_cuda.dll')));
    assert.ok(fs.existsSync(path.join(stagedDir, 'onnxruntime_providers_shared.dll')));
    assert.ok(fs.existsSync(path.join(stagedDir, 'ggml-cuda.dll')));
    assert.ok(fs.existsSync(path.join(stagedDir, 'cudart64_12.dll')));
    assert.ok(fs.existsSync(path.join(stagedDir, 'version.txt')));
    assert.ok(fs.existsSync(path.join(stagedDir, 'cuda-addon-manifest.json')));
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});

test('packageCudaAddonArchive creates package and sha256 checksum', () => {
  const testBaseDir = path.join(repoRoot, 'target', 'test-temp');
  fs.mkdirSync(testBaseDir, { recursive: true });
  const tempRoot = fs.mkdtempSync(path.join(testBaseDir, 'cuda-package-'));
  try {
    const stagedDir = path.join(tempRoot, 'staged');
    fs.mkdirSync(stagedDir, { recursive: true });
    fs.writeFileSync(path.join(stagedDir, 'dummy.dll'), 'hello cuda');

    const archivePath = path.join(tempRoot, 'test-addon.tar.gz');
    const info = packageCudaAddonArchive({ stagedDir, archivePath, platform: 'win32' });

    assert.ok(fs.existsSync(archivePath));
    assert.ok(fs.existsSync(`${archivePath}.sha256`));
    assert.equal(typeof info.sha256, 'string');
    assert.equal(info.sha256.length, 64);
    assert.ok(info.sizeBytes > 0);

    const shaContent = fs.readFileSync(`${archivePath}.sha256`, 'utf8');
    assert.ok(shaContent.includes(info.sha256));
    assert.ok(shaContent.includes('test-addon.tar.gz'));
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});

test('downloadLlamaCppCuda returns null on unsupported platform', () => {
  const cacheDir = path.join(repoRoot, 'target', 'mock-cache');
  assert.equal(downloadLlamaCppCuda({ cacheDir, platform: 'darwin' }), null);
});

test('stageCudaAddonFiles discovers and copies libraries in nested subdirectories', () => {
  const testBaseDir = path.join(repoRoot, 'target', 'test-temp');
  fs.mkdirSync(testBaseDir, { recursive: true });
  const tempRoot = fs.mkdtempSync(path.join(testBaseDir, 'cuda-nested-stage-'));
  try {
    const llamaBuildDir = path.join(tempRoot, 'llama-cpp-src', 'build', 'ggml', 'src', 'ggml-cuda');
    const stagedDir = path.join(tempRoot, 'staged');
    fs.mkdirSync(llamaBuildDir, { recursive: true });
    fs.writeFileSync(path.join(llamaBuildDir, 'libggml-cuda.so'), 'fake ggml cuda so');

    const copied = stageCudaAddonFiles({
      stagedDir,
      llamaLibDir: path.join(tempRoot, 'llama-cpp-src', 'build'),
      platform: 'linux',
      version: '0.1.0',
    });

    assert.ok(copied.includes('libggml-cuda.so'));
    assert.ok(fs.existsSync(path.join(stagedDir, 'libggml-cuda.so')));
    assert.equal(fs.readFileSync(path.join(stagedDir, 'libggml-cuda.so'), 'utf8'), 'fake ggml cuda so');
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});
