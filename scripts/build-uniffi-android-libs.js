#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  loadAndroidSherpaSource,
  prepareAndroidSherpaRuntime,
  stageAndroidSherpaRuntime,
} from './android-sherpa-runtime.js';

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), '..');
const args = process.argv.slice(2);
const sherpaSourceLockPath = path.join(
  repoRoot,
  'platforms',
  'android',
  'packaging',
  'sherpa-onnx-sources.json',
);
const sherpaSource = loadAndroidSherpaSource(sherpaSourceLockPath);

const ABI_TARGETS = {
  'arm64-v8a': 'aarch64-linux-android',
  'armeabi-v7a': 'armv7-linux-androideabi',
  'x86': 'i686-linux-android',
  'x86_64': 'x86_64-linux-android',
};

const LINKER_PREFIXES = {
  'aarch64-linux-android': 'aarch64-linux-android',
  'armv7-linux-androideabi': 'armv7a-linux-androideabi',
  'i686-linux-android': 'i686-linux-android',
  'x86_64-linux-android': 'x86_64-linux-android',
};

function readOption(name, fallback) {
  const index = args.indexOf(name);
  if (index === -1) {
    return fallback;
  }
  const value = args[index + 1];
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function splitList(value) {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function selectedAbis() {
  const requested = readOption('--abis', process.env.SONA_ANDROID_ABIS ?? Object.keys(ABI_TARGETS).join(','));
  const abis = splitList(requested);
  if (abis.length === 0) {
    throw new Error('At least one Android ABI is required');
  }
  for (const abi of abis) {
    if (!ABI_TARGETS[abi]) {
      throw new Error(`Unsupported Android ABI '${abi}'. Supported ABIs: ${Object.keys(ABI_TARGETS).join(', ')}`);
    }
  }
  return abis;
}

function androidHostToolchain(hostPlatform) {
  if (hostPlatform === 'windows' || hostPlatform === 'win32') {
    return {
      hostTag: 'windows-x86_64',
      linkerExtension: '.cmd',
      archiverExtension: '.exe',
    };
  }
  if (hostPlatform === 'linux') {
    return {
      hostTag: 'linux-x86_64',
      linkerExtension: '',
      archiverExtension: '',
    };
  }
  if (hostPlatform === 'darwin') {
    return {
      hostTag: 'darwin-x86_64',
      linkerExtension: '',
      archiverExtension: '',
    };
  }
  throw new Error(`Unsupported Android NDK host platform '${hostPlatform}'`);
}

function linkerPathForNdk(ndkHome, target, minSdk, hostPlatform) {
  const hostToolchain = androidHostToolchain(hostPlatform);
  const linkerName = `${LINKER_PREFIXES[target]}${minSdk}-clang${hostToolchain.linkerExtension}`;
  return path.join(ndkHome, 'toolchains', 'llvm', 'prebuilt', hostToolchain.hostTag, 'bin', linkerName);
}

function archiverPathForNdk(ndkHome, hostPlatform) {
  const hostToolchain = androidHostToolchain(hostPlatform);
  return path.join(
    ndkHome,
    'toolchains',
    'llvm',
    'prebuilt',
    hostToolchain.hostTag,
    'bin',
    `llvm-ar${hostToolchain.archiverExtension}`,
  );
}

function findAndroidNdkHome(target, minSdk, hostPlatform) {
  const explicit = process.env.ANDROID_NDK_HOME ?? process.env.ANDROID_NDK_ROOT;
  if (explicit) {
    return explicit;
  }

  const androidHome = process.env.ANDROID_HOME ?? process.env.ANDROID_SDK_ROOT;
  if (!androidHome) {
    return null;
  }

  const ndkRoot = path.join(androidHome, 'ndk');
  if (!fs.existsSync(ndkRoot)) {
    return null;
  }

  const versions = fs.readdirSync(ndkRoot)
    .filter((entry) => fs.statSync(path.join(ndkRoot, entry)).isDirectory())
    .sort((left, right) => right.localeCompare(left, undefined, { numeric: true }));
  return versions
    .map((version) => path.join(ndkRoot, version))
    .find((ndkHome) => (
      fs.existsSync(linkerPathForNdk(ndkHome, target, minSdk, hostPlatform))
      && fs.existsSync(archiverPathForNdk(ndkHome, hostPlatform))
    )) ?? null;
}

function toolchainEnvForTarget(target, minSdk, hostPlatform) {
  const ndkHome = findAndroidNdkHome(target, minSdk, hostPlatform);
  if (!ndkHome) {
    return {};
  }

  const linkerPath = linkerPathForNdk(ndkHome, target, minSdk, hostPlatform);
  const archiverPath = archiverPathForNdk(ndkHome, hostPlatform);
  if (!fs.existsSync(linkerPath)) {
    throw new Error(`Missing Android NDK linker at ${linkerPath}`);
  }
  if (!fs.existsSync(archiverPath)) {
    throw new Error(`Missing Android NDK archiver at ${archiverPath}`);
  }

  const targetEnvSuffix = target.replace(/-/gu, '_');
  const cargoLinkerEnvName = `CARGO_TARGET_${targetEnvSuffix.toUpperCase()}_LINKER`;
  return {
    [cargoLinkerEnvName]: linkerPath,
    [`CC_${targetEnvSuffix}`]: linkerPath,
    [`AR_${targetEnvSuffix}`]: archiverPath,
  };
}

function runCargoBuild(target, profile, minSdk, sherpaLibDir) {
  const releaseFlag = profile === 'release' ? ['--release'] : [];
  const cargo = process.env.CARGO ?? 'cargo';
  const env = {
    ...process.env,
    ...toolchainEnvForTarget(target, minSdk, process.platform),
    SHERPA_ONNX_LIB_DIR: sherpaLibDir,
  };
  const result = spawnSync(cargo, ['build', '-p', 'sona-uniffi-bind', '--target', target, ...releaseFlag], {
    cwd: repoRoot,
    env,
    stdio: 'inherit',
  });

  if (result.status !== 0) {
    throw new Error(`${cargo} build for ${target} failed with exit code ${result.status}`);
  }
}

function copyAndroidLibrary(targetDir, target, profile, abi, outDir) {
  const profileDir = profile === 'release' ? 'release' : 'debug';
  const source = path.join(targetDir, target, profileDir, 'libsona_uniffi_bind.so');
  if (!fs.existsSync(source)) {
    throw new Error(`Missing Android UniFFI library at ${source}`);
  }

  const destinationDir = path.join(outDir, abi);
  fs.mkdirSync(destinationDir, { recursive: true });
  fs.copyFileSync(source, path.join(destinationDir, 'libsona_uniffi_bind.so'));
}

function androidAarNativeEntries(abi) {
  return [
    'libsona_uniffi_bind.so',
    ...sherpaSource.runtimeLibraries,
  ].map((library) => `jni/${abi}/${library}`);
}

function formatAndroidBuildPlan(abi, target) {
  return `Plan: ${abi} -> ${target}; AAR native entries: ${androidAarNativeEntries(abi).join(', ')}`;
}

function prepareOutputDirectory(outDir) {
  fs.rmSync(outDir, { recursive: true, force: true });
  fs.mkdirSync(outDir, { recursive: true });
}

const profile = readOption('--profile', 'release');
const minSdk = readOption('--min-sdk', process.env.SONA_ANDROID_MIN_SDK ?? '23');
const targetDir = path.resolve(
  readOption('--target-dir', process.env.CARGO_TARGET_DIR ?? path.join(repoRoot, 'target')),
);
const outDir = path.resolve(
  readOption('--out-dir', path.join(repoRoot, 'platforms', 'android', 'generated', 'jniLibs', 'main')),
);
const dryRun = args.includes('--dry-run');
const printLinkerEnv = args.includes('--print-linker-env');
const hostPlatformOverride = readOption('--host-platform', null);

if (hostPlatformOverride && !printLinkerEnv) {
  throw new Error('--host-platform is only supported with --print-linker-env');
}
const printHostPlatform = hostPlatformOverride ?? process.platform;
const abis = selectedAbis();
const preparedSherpaRuntime = (!dryRun && !printLinkerEnv)
  ? await prepareAndroidSherpaRuntime({
    source: sherpaSource,
    cacheRoot: path.join(targetDir, 'android-sherpa'),
    selectedAbis: abis,
    archiveOverride: process.env.SONA_SHERPA_ONNX_ANDROID_ARCHIVE,
  })
  : null;

if (!dryRun && !printLinkerEnv) {
  prepareOutputDirectory(outDir);
}

for (const abi of abis) {
  const target = ABI_TARGETS[abi];
  if (printLinkerEnv) {
    console.error(formatAndroidBuildPlan(abi, target));
    const toolchainEnv = toolchainEnvForTarget(target, minSdk, printHostPlatform);
    for (const [name, value] of Object.entries(toolchainEnv)) {
      console.log(`${name}=${value}`);
    }
    continue;
  }
  if (dryRun) {
    console.log(formatAndroidBuildPlan(abi, target));
    continue;
  }
  const sherpaLibDir = path.join(preparedSherpaRuntime.rootDir, 'jniLibs', abi);
  runCargoBuild(target, profile, minSdk, sherpaLibDir);
  copyAndroidLibrary(targetDir, target, profile, abi, outDir);
  stageAndroidSherpaRuntime(preparedSherpaRuntime, abi, outDir);
}

if (!printLinkerEnv) {
  console.log(`${dryRun ? 'Planned' : 'Staged'} Sona UniFFI Android JNI libraries in ${outDir}`);
}
