#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyAndroidClientApk } from './android-client-apk.js';

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), '..');
const clientProjectDir = path.join(repoRoot, 'platforms', 'android', 'client');
const managedGradleRunner = path.join(repoRoot, 'scripts', 'run-managed-gradle.js');
const defaultAndroidAbis = ['arm64-v8a', 'x86_64'];
const apkFileNames = new Map([
  ['arm64-v8a', 'app-arm64-v8a-debug.apk'],
  ['x86_64', 'app-x86_64-debug.apk'],
]);

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? repoRoot,
    env: options.env ?? process.env,
    stdio: 'inherit',
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed with exit code ${result.status}`);
  }
}

function splitList(value) {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function selectedAndroidAbis(value) {
  const abis = [...new Set(splitList(value))];
  if (abis.length === 0) {
    throw new Error('SONA_ANDROID_ABIS must select at least one Android ABI');
  }
  for (const abi of abis) {
    if (!apkFileNames.has(abi)) {
      throw new Error(`Android client does not deliver an APK for ABI ${abi}`);
    }
  }
  return abis;
}

const defaultAndroidAbiList = defaultAndroidAbis.join(',');
const gradleEnv = {
  ...process.env,
  SONA_ANDROID_ABIS: process.env.SONA_ANDROID_ABIS ?? 'arm64-v8a,x86_64',
};
const androidAbis = selectedAndroidAbis(gradleEnv.SONA_ANDROID_ABIS ?? defaultAndroidAbiList);

run(process.execPath, [
  managedGradleRunner,
  '--project-dir',
  clientProjectDir,
  '--',
  '--no-daemon',
  ':application:testDebugUnitTest',
  ':app:assembleDebug',
  ':app:lintDebug',
  '--quiet',
], { env: gradleEnv });

const apkOutputDir = path.join(clientProjectDir, 'app', 'build', 'outputs', 'apk', 'debug');
for (const abi of androidAbis) {
  const apkPath = path.join(apkOutputDir, apkFileNames.get(abi));
  verifyAndroidClientApk(apkPath, abi);
  console.log(`Verified Sona Android ${abi} client at ${apkPath}`);
}
