import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { repoRoot } from './repository.js';

const node = process.execPath;
const androidNdkAbiCases = [
  {
    abi: 'arm64-v8a',
    target: 'aarch64-linux-android',
    linkerPrefix: 'aarch64-linux-android',
  },
  {
    abi: 'armeabi-v7a',
    target: 'armv7-linux-androideabi',
    linkerPrefix: 'armv7a-linux-androideabi',
  },
  {
    abi: 'x86',
    target: 'i686-linux-android',
    linkerPrefix: 'i686-linux-android',
  },
  {
    abi: 'x86_64',
    target: 'x86_64-linux-android',
    linkerPrefix: 'x86_64-linux-android',
  },
];

function androidNdkHostLayout(hostPlatform) {
  if (hostPlatform === 'windows' || hostPlatform === 'win32') {
    return { hostTag: 'windows-x86_64', linkerExtension: '.cmd', archiverExtension: '.exe' };
  }
  if (hostPlatform === 'darwin') {
    return { hostTag: 'darwin-x86_64', linkerExtension: '', archiverExtension: '' };
  }
  return { hostTag: 'linux-x86_64', linkerExtension: '', archiverExtension: '' };
}

function androidNdkToolPaths(ndkHome, abiCase, hostPlatform) {
  const layout = androidNdkHostLayout(hostPlatform);
  const binDir = path.join(ndkHome, 'toolchains', 'llvm', 'prebuilt', layout.hostTag, 'bin');
  return {
    linkerPath: path.join(binDir, `${abiCase.linkerPrefix}23-clang${layout.linkerExtension}`),
    archiverPath: path.join(binDir, `llvm-ar${layout.archiverExtension}`),
  };
}

function runAndroidNdkPrint({
  abi,
  androidHome = '',
  ndkHome = '',
  hostPlatform,
  targetDir,
  outDir,
  archiveOverride,
}) {
  const commandArgs = [
    path.join(repoRoot, 'scripts', 'build-uniffi-android-libs.js'),
    '--print-linker-env',
    '--abis',
    abi,
  ];
  if (hostPlatform) {
    commandArgs.push('--host-platform', hostPlatform);
  }
  if (targetDir) {
    commandArgs.push('--target-dir', targetDir);
  }
  if (outDir) {
    commandArgs.push('--out-dir', outDir);
  }

  return spawnSync(node, commandArgs, {
    cwd: repoRoot,
    encoding: 'utf8',
    env: {
      ...process.env,
      ANDROID_HOME: androidHome,
      ANDROID_SDK_ROOT: '',
      ANDROID_NDK_HOME: ndkHome,
      ANDROID_NDK_ROOT: '',
      SONA_SHERPA_ONNX_ANDROID_ARCHIVE: archiveOverride ?? '',
    },
  });
}

export { node, androidNdkAbiCases, androidNdkToolPaths, runAndroidNdkPrint };
