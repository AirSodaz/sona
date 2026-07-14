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
const supportedAndroidAbis = new Set(defaultAndroidAbis);
const buildRelease = process.env.SONA_ANDROID_BUILD_RELEASE === 'true';
const apkVariants = [
  { outputDir: 'debug', fileSuffix: 'debug' },
];
if (buildRelease) {
  apkVariants.push({ outputDir: 'release', fileSuffix: 'release-unsigned' });
}

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
    if (!supportedAndroidAbis.has(abi)) {
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

const gradleArgs = [
  '--no-daemon',
  ':application:testDebugUnitTest',
  ':adapters:android:testDebugUnitTest',
  ':adapters:uniffi:testDebugUnitTest',
  ':adapters:android:assembleDebugAndroidTest',
  ':adapters:android:lintDebug',
  ':app:testDebugUnitTest',
  ':app:assembleDebug',
  ':app:lintDebug',
];
if (buildRelease) {
  gradleArgs.push(':app:assembleRelease');
}

run(process.execPath, [
  managedGradleRunner,
  '--project-dir',
  clientProjectDir,
  '--',
  ...gradleArgs,
  '--quiet',
], { env: gradleEnv });

for (const variant of apkVariants) {
  const apkOutputDir = path.join(
    clientProjectDir,
    'app',
    'build',
    'outputs',
    'apk',
    variant.outputDir,
  );
  for (const abi of androidAbis) {
    const apkPath = path.join(apkOutputDir, `app-${abi}-${variant.fileSuffix}.apk`);
    verifyAndroidClientApk(apkPath, abi);
    console.log(`Verified Sona Android ${abi} ${variant.outputDir} client at ${apkPath}`);
  }
}
