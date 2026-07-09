#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), '..');
const args = process.argv.slice(2);

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
  for (const abi of abis) {
    if (!ABI_TARGETS[abi]) {
      throw new Error(`Unsupported Android ABI '${abi}'. Supported ABIs: ${Object.keys(ABI_TARGETS).join(', ')}`);
    }
  }
  return abis;
}

function androidHostTag() {
  return process.platform === 'win32'
    ? 'windows-x86_64'
    : process.platform === 'darwin'
      ? 'darwin-x86_64'
      : 'linux-x86_64';
}

function linkerPathForNdk(ndkHome, target, minSdk) {
  const extension = process.platform === 'win32' ? '.cmd' : '';
  const linkerName = `${LINKER_PREFIXES[target]}${minSdk}-clang${extension}`;
  return path.join(ndkHome, 'toolchains', 'llvm', 'prebuilt', androidHostTag(), 'bin', linkerName);
}

function findAndroidNdkHome(target, minSdk) {
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
    .find((ndkHome) => fs.existsSync(linkerPathForNdk(ndkHome, target, minSdk))) ?? null;
}

function linkerEnvForTarget(target, minSdk) {
  const ndkHome = findAndroidNdkHome(target, minSdk);
  if (!ndkHome) {
    return {};
  }

  const linkerPath = linkerPathForNdk(ndkHome, target, minSdk);
  if (!fs.existsSync(linkerPath)) {
    throw new Error(`Missing Android NDK linker at ${linkerPath}`);
  }

  const envName = `CARGO_TARGET_${target.toUpperCase().replace(/-/gu, '_')}_LINKER`;
  return { [envName]: linkerPath };
}

function runCargoBuild(target, profile, minSdk) {
  const releaseFlag = profile === 'release' ? ['--release'] : [];
  const cargo = process.env.CARGO ?? 'cargo';
  const env = {
    ...process.env,
    ...linkerEnvForTarget(target, minSdk),
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

function androidLibraryPath(targetDir, target, profile) {
  const profileDir = profile === 'release' ? 'release' : 'debug';
  return path.join(targetDir, target, profileDir, 'libsona_uniffi_bind.so');
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

if (!dryRun && !printLinkerEnv) {
  prepareOutputDirectory(outDir);
}

for (const abi of selectedAbis()) {
  const target = ABI_TARGETS[abi];
  if (printLinkerEnv) {
    const linkerEnv = linkerEnvForTarget(target, minSdk);
    for (const [name, value] of Object.entries(linkerEnv)) {
      console.log(`${name}=${value}`);
    }
    continue;
  }
  if (dryRun) {
    const source = androidLibraryPath(targetDir, target, profile);
    const destination = path.join(outDir, abi, 'libsona_uniffi_bind.so');
    console.log(`Dry run: ${abi} -> ${target}; ${source} => ${destination}`);
    continue;
  }
  runCargoBuild(target, profile, minSdk);
  copyAndroidLibrary(targetDir, target, profile, abi, outDir);
}

if (!printLinkerEnv) {
  console.log(`${dryRun ? 'Planned' : 'Staged'} Sona UniFFI Android JNI libraries in ${outDir}`);
}
