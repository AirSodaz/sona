import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { gzipSync } from 'node:zlib';
import { repoRoot } from './repo-root.js';

const node = process.execPath;
function makeTempRepo() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-packaging-'));
  fs.mkdirSync(path.join(root, 'platforms', 'desktop', 'binaries'), { recursive: true });
  fs.mkdirSync(path.join(root, 'platforms', 'desktop', 'resources', 'cli'), { recursive: true });
  fs.mkdirSync(path.join(root, 'platforms', 'desktop', 'resources', 'shared_libs'), { recursive: true });
  return root;
}

function writeNativeFfmpegBinary(target) {
  if (target.includes('apple')) {
    const binary = Buffer.alloc(0x20);
    binary.writeUInt32LE(0xfeedfacf, 0);
    binary.writeUInt32LE(target.includes('aarch64') ? 0x0100000c : 0x01000007, 4);
    binary.writeUInt32LE(2, 12);
    return binary;
  }
  if (target.includes('linux')) {
    const binary = Buffer.alloc(0x40);
    binary.write('\x7fELF', 0, 'latin1');
    binary.writeUInt8(2, 4);
    binary.writeUInt8(1, 5);
    binary.writeUInt16LE(2, 16);
    binary.writeUInt16LE(target.includes('aarch64') ? 0xb7 : 0x3e, 18);
    return binary;
  }
  const binary = Buffer.alloc(0x200);
  binary.write('MZ', 0, 'ascii');
  binary.writeUInt32LE(0x80, 0x3c);
  binary.write('PE\0\0', 0x80, 'ascii');
  binary.writeUInt16LE(target.includes('aarch64') ? 0xaa64 : 0x8664, 0x84);
  binary.writeUInt16LE(0xf0, 0x94);
  binary.writeUInt16LE(0x20b, 0x98);
  return binary;
}

function runtimeLibraryNames(target) {
  if (target.includes('windows')) return ['sherpa-onnx-c-api.dll', 'onnxruntime.dll', 'optional-runtime.dll'];
  if (target.includes('apple')) return ['libsherpa-onnx-c-api.dylib', 'libonnxruntime.dylib'];
  return ['libsherpa-onnx-c-api.so.1', 'libonnxruntime.so.1'];
}

function writeRuntimeLibraries(directoryPath, target) {
  fs.mkdirSync(directoryPath, { recursive: true });
  for (const libraryName of runtimeLibraryNames(target)) {
    fs.writeFileSync(path.join(directoryPath, libraryName), libraryName);
  }
}

function runtimeFileMap(runtimeLibDir, target, destinationDir) {
  return Object.fromEntries(
    runtimeLibraryNames(target).map((libraryName) => [
      `${destinationDir}/${libraryName}`,
      path.join(runtimeLibDir, libraryName),
    ]),
  );
}

async function prepareBundleFixture(target) {
  const { prepareDesktopBundle } = await loadDesktopBundlePreparer();
  const root = makeTempRepo();
  const releaseDir = path.join(root, 'target', target, 'release');
  const runtimeLibDir = path.join(root, 'native-libs');
  const configPath = path.join(root, 'base-tauri.conf.json');
  fs.mkdirSync(releaseDir, { recursive: true });
  fs.writeFileSync(path.join(releaseDir, target.includes('windows') ? 'sona-cli.exe' : 'sona-cli'), 'cli');
  writeRuntimeLibraries(runtimeLibDir, target);
  fs.writeFileSync(configPath, JSON.stringify({ bundle: {} }));

  return prepareDesktopBundle({
    repoRoot: root,
    target,
    configPath,
    ffmpegLockPath: writeTestFfmpegLock(root, target),
    sherpaLibDir: runtimeLibDir,
    runCommand() {},
    readMacDylibDependencies() { return []; },
  });
}

function writeGeneratedBundleConfig(configPath, target, sidecarsDir, runtimeLibDir) {
  const config = {
    bundle: {
      externalBin: [path.join(sidecarsDir, 'sona-cli'), path.join(sidecarsDir, 'ffmpeg')],
    },
  };
  if (target.includes('windows')) {
    config.bundle.resources = { [path.join(runtimeLibDir, '*')]: '' };
  } else if (target.includes('apple')) {
    config.bundle.macOS = { files: runtimeFileMap(runtimeLibDir, target, 'Frameworks') };
  } else {
    const files = runtimeFileMap(runtimeLibDir, target, './usr/lib/sona');
    config.bundle.linux = {
      deb: { files },
      rpm: { files },
      appimage: { files },
    };
  }
  fs.writeFileSync(configPath, JSON.stringify(config));
}

function writeCanonicalAppBundle(root, target, releaseDir = path.join(root, 'target', target, 'release')) {
  const bundleRoot = path.join(releaseDir, 'bundle');
  fs.mkdirSync(bundleRoot, { recursive: true });
  if (target.includes('windows')) {
    fs.writeFileSync(path.join(releaseDir, 'sona.exe'), 'app');
    fs.writeFileSync(path.join(releaseDir, 'sona-cli.exe'), 'cli');
    fs.writeFileSync(path.join(releaseDir, 'ffmpeg.exe'), 'ffmpeg');
    for (const libraryName of runtimeLibraryNames(target)) fs.writeFileSync(path.join(releaseDir, libraryName), libraryName);
    const installerPath = path.join(bundleRoot, 'nsis', 'Sona_0.8.0_x64-setup.exe');
    fs.mkdirSync(path.dirname(installerPath), { recursive: true });
    fs.writeFileSync(installerPath, 'installer');
    return;
  }
  if (target.includes('apple')) {
    const contents = path.join(bundleRoot, 'macos', 'Sona.app', 'Contents');
    const macosDir = path.join(contents, 'MacOS');
    const frameworksDir = path.join(contents, 'Frameworks');
    fs.mkdirSync(macosDir, { recursive: true });
    fs.mkdirSync(frameworksDir, { recursive: true });
    fs.writeFileSync(path.join(macosDir, 'sona'), 'app');
    fs.writeFileSync(path.join(macosDir, 'sona-cli'), 'cli');
    fs.writeFileSync(path.join(macosDir, 'ffmpeg'), 'ffmpeg');
    for (const libraryName of runtimeLibraryNames(target)) fs.writeFileSync(path.join(frameworksDir, libraryName), libraryName);
    const dmgPath = path.join(bundleRoot, 'dmg', 'Sona_0.8.0_aarch64.dmg');
    fs.mkdirSync(path.dirname(dmgPath), { recursive: true });
    fs.writeFileSync(dmgPath, 'installer');
    return;
  }
  const appRoot = path.join(bundleRoot, 'appimage', 'Sona.AppDir');
  const binDir = path.join(appRoot, 'usr', 'bin');
  const libDir = path.join(appRoot, 'usr', 'lib', 'sona');
  fs.mkdirSync(binDir, { recursive: true });
  fs.mkdirSync(libDir, { recursive: true });
  fs.writeFileSync(path.join(binDir, 'sona'), 'app');
  fs.writeFileSync(path.join(binDir, 'sona-cli'), 'cli');
  fs.writeFileSync(path.join(binDir, 'ffmpeg'), 'ffmpeg');
  for (const libraryName of runtimeLibraryNames(target)) fs.writeFileSync(path.join(libDir, libraryName), libraryName);
  fs.writeFileSync(path.join(bundleRoot, 'appimage', 'Sona_0.8.0_amd64.AppImage'), 'installer');
}

function writeTauriWrapperStubs(root) {
  const logPath = path.join(root, 'tauri-args.json');
  const loggerPath = path.join(root, 'tauri-logger.mjs');
  const preparerPath = path.join(root, 'prepare-stub.mjs');
  fs.writeFileSync(
    loggerPath,
    "import fs from 'node:fs'; fs.writeFileSync(process.env.SONA_TAURI_ARGS_LOG, JSON.stringify({ args: process.argv.slice(2), sherpaLibDir: process.env.SHERPA_ONNX_LIB_DIR }));\n",
  );
  fs.writeFileSync(
    preparerPath,
    "import fs from 'node:fs'; import path from 'node:path'; const args = process.argv.slice(2); const value = (flag) => args[args.indexOf(flag) + 1]; const root = value('--repo-root'); const target = value('--target'); const config = path.join(root, 'target', 'desktop-bundle', target, 'tauri.bundle.conf.json'); fs.mkdirSync(path.dirname(config), { recursive: true }); fs.writeFileSync(config, '{}');\n",
  );

  if (process.platform === 'win32') {
    const tauriBinary = path.join(root, 'tauri-stub.cmd');
    fs.writeFileSync(tauriBinary, `@echo off\r\n"${process.execPath}" "${loggerPath}" %*\r\n`);
    return { logPath, preparerPath, tauriBinary };
  }

  const tauriBinary = path.join(root, 'tauri-stub.sh');
  fs.writeFileSync(tauriBinary, `#!/bin/sh\n"${process.execPath}" "${loggerPath}" "$@"\n`);
  fs.chmodSync(tauriBinary, 0o755);
  return { logPath, preparerPath, tauriBinary };
}

function writeTestFfmpegLock(root, target) {
  const binaryPath = target.includes('windows') ? 'ffmpeg.exe' : 'ffmpeg';
  const archivePath = path.join(root, 'ffmpeg-test.gz');
  const binary = writeNativeFfmpegBinary(target);
  const archive = gzipSync(binary);
  fs.writeFileSync(archivePath, archive);

  const lockPath = path.join(root, 'ffmpeg-sources.json');
  fs.writeFileSync(
    lockPath,
    JSON.stringify({
      sources: [{
        target,
        url: new URL(`file://${archivePath.replaceAll('\\', '/')}`).href,
        archiveKind: 'gz',
        binaryPath,
        sha256: createHash('sha256').update(archive).digest('hex'),
        license: 'GPL-3.0-or-later',
      }],
    }),
  );
  return lockPath;
}

async function loadDesktopBundlePreparer() {
  return import(pathToFileURL(path.join(repoRoot, 'platforms', 'desktop', 'scripts', 'prepare-desktop-bundle.js')).href);
}

export { node, makeTempRepo, writeNativeFfmpegBinary, runtimeLibraryNames, writeRuntimeLibraries, runtimeFileMap, prepareBundleFixture, writeGeneratedBundleConfig, writeCanonicalAppBundle, writeTauriWrapperStubs, writeTestFfmpegLock, loadDesktopBundlePreparer };
