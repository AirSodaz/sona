#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createCudaAddonManifest } from './cuda-addon-manifest.js';

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), '..');

const REQUIRED_FILES = {
  'windows-x86_64': [
    'onnxruntime_providers_cuda.dll',
    'onnxruntime_providers_shared.dll',
    'ggml-cuda.dll',
    'cudart64_12.dll',
    'cublas64_12.dll',
    'cublasLt64_12.dll',
  ],
  'linux-x86_64': [
    'libonnxruntime_providers_cuda.so',
    'libonnxruntime_providers_shared.so',
    'libggml-cuda.so',
    'libcudart.so.12',
    'libcublas.so.12',
    'libcublasLt.so.12',
  ],
};

function normalizePlatform(platform = process.platform) {
  if (platform === 'win32') return 'windows-x86_64';
  if (platform === 'linux') return 'linux-x86_64';
  return platform;
}

function resolveLocalCudaRuntimeDir(platform = process.platform, env = process.env) {
  if (platform === 'win32') {
    const localAppData = env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local');
    const primary = path.join(localAppData, 'com.asoda.sona', 'runtimes', 'cuda');
    if (fs.existsSync(path.dirname(primary))) return primary;
    const fallback = path.join(localAppData, 'Sona', 'runtimes', 'cuda');
    if (fs.existsSync(path.dirname(fallback))) return fallback;
    return primary;
  }
  if (platform === 'linux') {
    const dataHome = env.XDG_DATA_HOME || path.join(os.homedir(), '.local', 'share');
    const primary = path.join(dataHome, 'com.asoda.sona', 'runtimes', 'cuda');
    if (fs.existsSync(path.dirname(primary))) return primary;
    const fallback = path.join(dataHome, 'Sona', 'runtimes', 'cuda');
    if (fs.existsSync(path.dirname(fallback))) return fallback;
    return primary;
  }
  return null;
}

function detectCudaEnvironment(env = process.env, platform = process.platform) {
  const normPlatform = normalizePlatform(platform);
  const isSupported = normPlatform === 'windows-x86_64' || normPlatform === 'linux-x86_64';

  let cudaPath = null;
  if (platform === 'win32') {
    cudaPath = env.CUDA_PATH || null;
  } else if (platform === 'linux') {
    if (env.CUDA_PATH && fs.existsSync(env.CUDA_PATH)) {
      cudaPath = env.CUDA_PATH;
    } else if (fs.existsSync('/usr/local/cuda')) {
      cudaPath = '/usr/local/cuda';
    }
  }

  let hasNvcc = false;
  let cudaVersion = null;
  const nvccCheck = spawnSync('nvcc', ['--version'], { encoding: 'utf8' });
  if (nvccCheck.status === 0) {
    hasNvcc = true;
    const match = /release\s+(?<ver>\d+\.\d+)/u.exec(nvccCheck.stdout);
    if (match?.groups?.ver) {
      cudaVersion = match.groups.ver;
    }
  }

  let hasCmake = false;
  const cmakeCheck = spawnSync('cmake', ['--version'], { encoding: 'utf8' });
  if (cmakeCheck.status === 0) {
    hasCmake = true;
  }

  return {
    supported: isSupported,
    platform: normPlatform,
    cudaPath,
    cudaVersion: cudaVersion || '12.4',
    hasNvcc,
    hasCmake,
  };
}

function collectCudaToolkitLibraries(cudaPath, platform = process.platform) {
  if (!cudaPath || !fs.existsSync(cudaPath)) {
    return [];
  }

  const collected = [];
  if (platform === 'win32') {
    const binDir = path.join(cudaPath, 'bin');
    if (fs.existsSync(binDir)) {
      const files = fs.readdirSync(binDir);
      for (const file of files) {
        if (/^(?:cudart64_|cublas64_|cublasLt64_).*\.dll$/iu.test(file)) {
          collected.push(path.join(binDir, file));
        }
      }
    }
  } else if (platform === 'linux') {
    const libDir = path.join(cudaPath, 'lib64');
    if (fs.existsSync(libDir)) {
      const files = fs.readdirSync(libDir);
      for (const file of files) {
        if (/^lib(?:cudart|cublas|cublasLt)\.so(?:\.\d+)*$/u.test(file)) {
          collected.push(path.join(libDir, file));
        }
      }
    }
  }

  return collected;
}

function downloadOnnxRuntimeGpu({
  version = '1.20.1',
  cacheDir,
  platform = process.platform,
}) {
  const normPlatform = normalizePlatform(platform);
  fs.mkdirSync(cacheDir, { recursive: true });

  if (normPlatform === 'windows-x86_64') {
    const archiveName = `onnxruntime-win-x64-gpu-${version}.zip`;
    const archivePath = path.join(cacheDir, archiveName);
    const extractDir = path.join(cacheDir, `ort-gpu-win-${version}`);
    const url = `https://github.com/microsoft/onnxruntime/releases/download/v${version}/${archiveName}`;

    if (!fs.existsSync(extractDir)) {
      console.log(`[ORT GPU] Downloading from ${url}...`);
      const curlRes = spawnSync('curl.exe', ['-sSfL', '--retry', '3', url, '-o', archivePath], {
        stdio: 'inherit',
      });
      if (curlRes.status !== 0) {
        throw new Error(`Failed to download ONNX Runtime GPU from ${url}`);
      }

      console.log(`[ORT GPU] Extracting ${archiveName}...`);
      const powershellCmd = `Expand-Archive -Path "${archivePath}" -DestinationPath "${extractDir}" -Force`;
      spawnSync('powershell', ['-NoProfile', '-Command', powershellCmd], { stdio: 'inherit' });
      fs.rmSync(archivePath, { force: true });
    }

    const libSubdir = fs.readdirSync(extractDir).find((dir) => dir.startsWith('onnxruntime-win-x64-gpu'));
    return libSubdir ? path.join(extractDir, libSubdir, 'lib') : path.join(extractDir, 'lib');
  }

  if (normPlatform === 'linux-x86_64') {
    const archiveName = `onnxruntime-linux-x64-gpu-${version}.tgz`;
    const archivePath = path.join(cacheDir, archiveName);
    const extractDir = path.join(cacheDir, `ort-gpu-linux-${version}`);
    const url = `https://github.com/microsoft/onnxruntime/releases/download/v${version}/${archiveName}`;

    if (!fs.existsSync(extractDir)) {
      console.log(`[ORT GPU] Downloading from ${url}...`);
      const wgetRes = spawnSync('wget', ['-q', url, '-O', archivePath], { stdio: 'inherit' });
      if (wgetRes.status !== 0) {
        throw new Error(`Failed to download ONNX Runtime GPU from ${url}`);
      }

      console.log(`[ORT GPU] Extracting ${archiveName}...`);
      fs.mkdirSync(extractDir, { recursive: true });
      spawnSync('tar', ['-xzf', archivePath, '-C', extractDir], { stdio: 'inherit' });
      fs.rmSync(archivePath, { force: true });
    }

    const libSubdir = fs.readdirSync(extractDir).find((dir) => dir.startsWith('onnxruntime-linux-x64-gpu'));
    return libSubdir ? path.join(extractDir, libSubdir, 'lib') : path.join(extractDir, 'lib');
  }

  throw new Error(`Unsupported platform for ONNX Runtime GPU: ${platform}`);
}

function buildLlamaCppGgmlCuda({
  llamaRef = 'b4500',
  buildRoot,
  cudaArchitectures = '60;70;75;80;86;89;90',
  platform = process.platform,
}) {
  const env = detectCudaEnvironment();
  if (!env.hasCmake) {
    throw new Error('CMake is required to build llama.cpp ggml-cuda');
  }
  if (!env.hasNvcc && !env.cudaPath) {
    throw new Error('NVIDIA CUDA Toolkit (nvcc) is required to build ggml-cuda');
  }

  fs.mkdirSync(buildRoot, { recursive: true });
  const srcDir = path.join(buildRoot, 'llama-cpp-src');

  if (!fs.existsSync(srcDir)) {
    console.log(`[llama.cpp] Cloning https://github.com/ggerganov/llama.cpp (ref: ${llamaRef})...`);
    const cloneRes = spawnSync(
      'git',
      ['clone', '--depth', '1', '--branch', llamaRef, 'https://github.com/ggerganov/llama.cpp.git', srcDir],
      { stdio: 'inherit' },
    );
    if (cloneRes.status !== 0) {
      throw new Error(`Failed to clone llama.cpp at ${llamaRef}`);
    }
  }

  const buildDir = path.join(srcDir, 'build');
  fs.mkdirSync(buildDir, { recursive: true });

  console.log('[llama.cpp] Configuring CMake with GGML_CUDA=ON...');
  const cmakeConfigArgs = [
    '-B',
    buildDir,
    '-S',
    srcDir,
    '-DCMAKE_BUILD_TYPE=Release',
    '-DGGML_CUDA=ON',
    '-DBUILD_SHARED_LIBS=ON',
    `-DCMAKE_CUDA_ARCHITECTURES=${cudaArchitectures}`,
  ];

  if (platform === 'win32') {
    cmakeConfigArgs.push('-G', 'Ninja');
  }

  const configRes = spawnSync('cmake', cmakeConfigArgs, { stdio: 'inherit' });
  if (configRes.status !== 0) {
    throw new Error('Failed to configure llama.cpp CMake build');
  }

  console.log('[llama.cpp] Building target ggml-cuda...');
  const buildRes = spawnSync('cmake', ['--build', buildDir, '--config', 'Release', '--target', 'ggml-cuda'], {
    stdio: 'inherit',
  });
  if (buildRes.status !== 0) {
    throw new Error('Failed to build ggml-cuda target');
  }

  const binDir = path.join(buildDir, 'bin');
  if (fs.existsSync(binDir)) {
    return binDir;
  }
  return buildDir;
}

function stageCudaAddonFiles({
  stagedDir,
  ortLibDir,
  llamaLibDir,
  cudaLibPaths = [],
  platform = process.platform,
  version = '0.1.0',
  cudaVersion = '12.4',
}) {
  fs.mkdirSync(stagedDir, { recursive: true });

  const copiedFiles = [];

  // 1. Copy ONNX Runtime GPU provider libraries
  if (ortLibDir && fs.existsSync(ortLibDir)) {
    const ortEntries = fs.readdirSync(ortLibDir);
    for (const file of ortEntries) {
      if (/onnxruntime_providers_(?:cuda|shared)/iu.test(file)) {
        const src = path.join(ortLibDir, file);
        const dest = path.join(stagedDir, file);
        fs.copyFileSync(src, dest);
        copiedFiles.push(file);
      }
    }
  }

  // 2. Copy ggml-cuda from llama.cpp build
  if (llamaLibDir && fs.existsSync(llamaLibDir)) {
    const llamaEntries = fs.readdirSync(llamaLibDir);
    for (const file of llamaEntries) {
      if (/ggml-cuda/iu.test(file)) {
        const src = path.join(llamaLibDir, file);
        const dest = path.join(stagedDir, file);
        fs.copyFileSync(src, dest);
        copiedFiles.push(file);
      }
    }
  }

  // 3. Copy CUDA runtime libraries
  for (const src of cudaLibPaths) {
    if (fs.existsSync(src)) {
      const filename = path.basename(src);
      const dest = path.join(stagedDir, filename);
      fs.copyFileSync(src, dest);
      copiedFiles.push(filename);
    }
  }
  // Write version and manifest
  const versionFile = path.join(stagedDir, 'version.txt');
  fs.writeFileSync(versionFile, `${version}\n`, 'utf8');
  copiedFiles.push('version.txt');

  const manifestFile = path.join(stagedDir, 'cuda-addon-manifest.json');
  const manifestData = {
    schemaVersion: '1.0',
    addonVersion: version,
    cudaVersion: cudaVersion || '12.4',
    targetOs: platform === 'win32' ? 'windows' : 'linux',
    targetArch: 'x86_64',
    publishedAt: new Date().toISOString(),
  };
  fs.writeFileSync(manifestFile, JSON.stringify(manifestData, null, 2), 'utf8');
  copiedFiles.push('cuda-addon-manifest.json');

  return copiedFiles;
}

function packageCudaAddonArchive({
  stagedDir,
  archivePath,
  platform = process.platform,
}) {
  fs.mkdirSync(path.dirname(archivePath), { recursive: true });

  const tarCmd = platform === 'win32' ? 'tar.exe' : 'tar';
  const tarRes = spawnSync(tarCmd, ['-czf', archivePath, '-C', stagedDir, '.'], { stdio: 'inherit' });
  if (tarRes.status !== 0) {
    throw new Error(`Failed to create tar.gz archive at ${archivePath}`);
  }
  const content = fs.readFileSync(archivePath);
  const hash = crypto.createHash('sha256').update(content).digest('hex');
  const filename = path.basename(archivePath);
  fs.writeFileSync(`${archivePath}.sha256`, `${hash}  ${filename}\n`, 'ascii');

  return {
    archivePath,
    sha256: hash,
    sizeBytes: fs.statSync(archivePath).size,
  };
}

function parseCliArgs(args) {
  const options = {};
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg.startsWith('--')) {
      const key = arg.slice(2);
      const nextArg = args[i + 1];
      if (nextArg && !nextArg.startsWith('--')) {
        options[key] = nextArg;
        i += 1;
      } else {
        options[key] = true;
      }
    }
  }
  return options;
}

function runCli() {
  const options = parseCliArgs(process.argv.slice(2));

  console.log('[CUDA Addon Builder] Inspecting build environment...');
  const env = detectCudaEnvironment();
  console.log(`  Platform: ${env.platform}`);
  console.log(`  Supported: ${env.supported ? 'YES' : 'NO'}`);
  console.log(`  CUDA Path: ${env.cudaPath || 'NOT DETECTED'}`);
  console.log(`  CUDA Version: ${env.cudaVersion}`);
  console.log(`  NVCC Available: ${env.hasNvcc ? 'YES' : 'NO'}`);
  console.log(`  CMake Available: ${env.hasCmake ? 'YES' : 'NO'}`);

  if (options['check-env']) {
    return;
  }

  if (!env.supported) {
    console.error(`CUDA Addon is not supported on platform: ${process.platform}`);
    process.exit(1);
  }

  const addonVersion = options['addon-version'] || '0.1.0';
  const cudaVersion = options['cuda-version'] || env.cudaVersion || '12.4';
  const ortVersion = options['ort-version'] || '1.20.1';
  const llamaRef = options['llama-ref'] || 'b4500';
  const outputDir = path.resolve(options['output-dir'] || path.join(repoRoot, 'dist', 'cuda-addon'));
  const cacheDir = path.join(repoRoot, 'target', 'cuda-addon-cache');
  const stageName = `sona-cuda-addon-v${addonVersion}-${process.platform === 'win32' ? 'windows-x64' : 'linux-x64'}`;
  const stagePath = path.join(outputDir, stageName);

  console.log(`[CUDA Addon Builder] Target addon version: ${addonVersion}`);
  console.log(`[CUDA Addon Builder] Staging output directory: ${stagePath}`);

  fs.mkdirSync(stagePath, { recursive: true });

  let ortLibDir = options['ort-lib-dir'] ? path.resolve(options['ort-lib-dir']) : null;
  if (!ortLibDir && options.download) {
    ortLibDir = downloadOnnxRuntimeGpu({ version: ortVersion, cacheDir });
  }

  let llamaLibDir = options['llama-lib-dir'] ? path.resolve(options['llama-lib-dir']) : null;
  if (!llamaLibDir && options.build) {
    llamaLibDir = buildLlamaCppGgmlCuda({ llamaRef, buildRoot: cacheDir });
  }

  const cudaLibs = collectCudaToolkitLibraries(env.cudaPath);
  console.log(`  Discovered ${cudaLibs.length} CUDA Toolkit runtime libraries.`);

  const copied = stageCudaAddonFiles({
    stagedDir: stagePath,
    ortLibDir,
    llamaLibDir,
    cudaLibPaths: cudaLibs,
  });
  console.log(`  Staged ${copied.length} libraries.`);

  // Generate manifest
  const manifest = createCudaAddonManifest({
    addonVersion,
    cudaVersion,
    artifactsDir: outputDir,
  });
  fs.writeFileSync(
    path.join(stagePath, 'cuda-addon-manifest.json'),
    JSON.stringify(manifest, null, 2),
    'utf8',
  );
  console.log('[CUDA Addon Builder] Staged manifest in addon directory.');

  if (options.package) {
    const ext = 'tar.gz';
    const archivePath = path.join(outputDir, `${stageName}.${ext}`);
    console.log(`[CUDA Addon Builder] Packaging archive to ${archivePath}...`);
    const archiveInfo = packageCudaAddonArchive({ stagedDir: stagePath, archivePath });
    console.log(`✓ Created archive: ${archiveInfo.archivePath} (${archiveInfo.sizeBytes} bytes, sha256: ${archiveInfo.sha256})`);
  }

  if (options['install-local']) {
    const localDir = resolveLocalCudaRuntimeDir();
    if (localDir) {
      console.log(`[CUDA Addon Builder] Installing directly to local directory: ${localDir}`);
      fs.mkdirSync(localDir, { recursive: true });
      const stagedFiles = fs.readdirSync(stagePath);
      for (const file of stagedFiles) {
        fs.copyFileSync(path.join(stagePath, file), path.join(localDir, file));
      }
      console.log('✓ Successfully installed CUDA addon to local runtime directory.');
    }
  }
  console.log('[CUDA Addon Builder] Done.');
}

export {
  REQUIRED_FILES,
  normalizePlatform,
  resolveLocalCudaRuntimeDir,
  detectCudaEnvironment,
  collectCudaToolkitLibraries,
  downloadOnnxRuntimeGpu,
  buildLlamaCppGgmlCuda,
  stageCudaAddonFiles,
  packageCudaAddonArchive,
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  runCli();
}
