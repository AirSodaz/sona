import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { repoRoot } from './test-support/repo-root.js';
import {
  loadDesktopBundlePreparer,
  makeTempRepo,
  prepareBundleFixture,
  runtimeFileMap,
  runtimeLibraryNames,
  writeNativeFfmpegBinary,
  writeRuntimeLibraries,
  writeTestFfmpegLock,
} from './test-support/desktop-packaging-fixtures.js';

test('desktop bundle preparer derives supported host targets and maps runtime libraries', async () => {
  const { resolveHostTarget } = await loadDesktopBundlePreparer();

  assert.equal(resolveHostTarget('win32', 'x64'), 'x86_64-pc-windows-msvc');
  assert.equal(resolveHostTarget('win32', 'arm64'), 'aarch64-pc-windows-msvc');
  assert.equal(resolveHostTarget('darwin', 'arm64'), 'aarch64-apple-darwin');
  assert.equal(resolveHostTarget('linux', 'x64'), 'x86_64-unknown-linux-gnu');
});

test('desktop bundle preparer rejects targets absent from its production source lock', async () => {
  const { prepareDesktopBundle } = await loadDesktopBundlePreparer();
  const target = 'x86_64-apple-darwin-unlisted';
  const root = makeTempRepo();
  const releaseDir = path.join(root, 'target', target, 'release');
  const runtimeLibDir = path.join(root, 'native-libs');
  const configPath = path.join(root, 'base-tauri.conf.json');
  const sourceLockPath = path.join(
    repoRoot,
    'platforms',
    'desktop',
    'packaging',
    'ffmpeg-sources.json',
  );

  fs.mkdirSync(releaseDir, { recursive: true });
  fs.writeFileSync(path.join(releaseDir, 'sona-cli'), 'cli');
  writeRuntimeLibraries(runtimeLibDir, target);
  fs.writeFileSync(configPath, JSON.stringify({ bundle: {} }));

  await assert.rejects(
    prepareDesktopBundle({
      repoRoot: root,
      target,
      configPath,
      ffmpegLockPath: sourceLockPath,
      sherpaLibDir: runtimeLibDir,
      runCommand() {},
      readMacDylibDependencies() { return []; },
    }),
    /No FFmpeg source lock entry exists for x86_64-apple-darwin-unlisted\./u,
  );
});

test('desktop bundle preparer stages target inputs and generates a replacement Tauri config', async () => {
  const { prepareDesktopBundle } = await loadDesktopBundlePreparer();
  const root = makeTempRepo();
  const target = 'x86_64-pc-windows-msvc';
  const releaseDir = path.join(root, 'target', target, 'release');
  const runtimeLibDir = path.join(root, 'native-libs');
  const configPath = path.join(root, 'base-tauri.conf.json');
  fs.mkdirSync(releaseDir, { recursive: true });
  fs.mkdirSync(runtimeLibDir, { recursive: true });
  fs.writeFileSync(path.join(releaseDir, 'sona-cli.exe'), 'cli');
  fs.writeFileSync(path.join(runtimeLibDir, 'sherpa-onnx-c-api.dll'), 'sherpa');
  fs.writeFileSync(path.join(runtimeLibDir, 'onnxruntime.dll'), 'onnxruntime');
  fs.writeFileSync(path.join(runtimeLibDir, 'optional-runtime.dll'), 'optional');
  fs.writeFileSync(
    configPath,
    JSON.stringify({
      bundle: {
        resources: ['resources/cli/*', 'resources/shared_libs/*'],
        externalBin: ['binaries/ffmpeg'],
      },
    }),
  );
  const cargoCalls = [];

  const prepared = await prepareDesktopBundle({
    repoRoot: root,
    target,
    configPath,
    ffmpegLockPath: writeTestFfmpegLock(root, target),
    sherpaLibDir: runtimeLibDir,
    runCommand(executable, commandArgs) {
      cargoCalls.push([executable, commandArgs]);
    },
  });

  assert.deepEqual(cargoCalls, [['cargo', ['build', '-p', 'sona-cli', '--release', '--target', target]]]);
  assert.equal(fs.existsSync(path.join(prepared.sidecarsDir, `sona-cli-${target}.exe`)), true);
  assert.equal(fs.existsSync(path.join(prepared.sidecarsDir, `ffmpeg-${target}.exe`)), true);
  assert.equal(fs.existsSync(path.join(prepared.runtimeLibDir, 'optional-runtime.dll')), true);

  const generatedConfig = JSON.parse(fs.readFileSync(prepared.configPath, 'utf8'));
  assert.deepEqual(generatedConfig.bundle.externalBin, [
    path.join(prepared.sidecarsDir, 'sona-cli'),
    path.join(prepared.sidecarsDir, 'ffmpeg'),
  ]);
  assert.deepEqual(generatedConfig.bundle.resources, {
    [path.join(prepared.runtimeLibDir, '*')]: '',
  });
});

test('desktop bundle preparer maps macOS and Linux runtime libraries through native bundle files', async () => {
  const macos = await prepareBundleFixture('aarch64-apple-darwin');
  const macosConfig = JSON.parse(fs.readFileSync(macos.configPath, 'utf8'));
  assert.equal(macosConfig.bundle.resources, undefined);
  assert.deepEqual(
    macosConfig.bundle.macOS.files,
    runtimeFileMap(macos.runtimeLibDir, macos.target, 'Frameworks'),
  );

  const linux = await prepareBundleFixture('x86_64-unknown-linux-gnu');
  const linuxConfig = JSON.parse(fs.readFileSync(linux.configPath, 'utf8'));
  assert.equal(linuxConfig.bundle.resources, undefined);
  for (const format of ['deb', 'rpm', 'appimage']) {
    assert.deepEqual(
      linuxConfig.bundle.linux[format].files,
      runtimeFileMap(linux.runtimeLibDir, linux.target, './usr/lib/sona'),
    );
    assert.equal(linuxConfig.bundle[format], undefined);
  }
});

test('desktop bundle file maps use Tauri destination-to-source semantics', async () => {
  const macos = await prepareBundleFixture('aarch64-apple-darwin');
  const macosConfig = JSON.parse(fs.readFileSync(macos.configPath, 'utf8'));
  const macosLibrary = runtimeLibraryNames(macos.target)[0];
  assert.equal(
    macosConfig.bundle.macOS.files[`Frameworks/${macosLibrary}`],
    path.join(macos.runtimeLibDir, macosLibrary),
  );

  const linux = await prepareBundleFixture('x86_64-unknown-linux-gnu');
  const linuxConfig = JSON.parse(fs.readFileSync(linux.configPath, 'utf8'));
  const linuxLibrary = runtimeLibraryNames(linux.target)[0];
  for (const format of ['deb', 'rpm', 'appimage']) {
    assert.equal(
      linuxConfig.bundle.linux[format].files[`./usr/lib/sona/${linuxLibrary}`],
      path.join(linux.runtimeLibDir, linuxLibrary),
    );
  }
});

test('desktop bundle preparer rebases staged macOS dylibs before linking the CLI', async () => {
  const { prepareDesktopBundle } = await loadDesktopBundlePreparer();
  const target = 'aarch64-apple-darwin';
  const root = makeTempRepo();
  const releaseDir = path.join(root, 'target', target, 'release');
  const sourceRuntimeLibDir = path.join(root, 'native-libs');
  const configPath = path.join(root, 'base-tauri.conf.json');
  const commandCalls = [];
  fs.mkdirSync(releaseDir, { recursive: true });
  fs.writeFileSync(path.join(releaseDir, 'sona-cli'), 'cli');
  writeRuntimeLibraries(sourceRuntimeLibDir, target);
  fs.writeFileSync(configPath, JSON.stringify({ bundle: {} }));

  const prepared = await prepareDesktopBundle({
    repoRoot: root,
    target,
    configPath,
    ffmpegLockPath: writeTestFfmpegLock(root, target),
    sherpaLibDir: sourceRuntimeLibDir,
    runCommand(executable, commandArgs, options = {}) {
      commandCalls.push({ executable, commandArgs, options });
    },
    readMacDylibDependencies(libraryPath) {
      return path.basename(libraryPath) === 'libsherpa-onnx-c-api.dylib'
        ? [path.join(sourceRuntimeLibDir, 'libonnxruntime.dylib')]
        : [];
    },
  });

  const stagedSherpa = path.join(prepared.runtimeLibDir, 'libsherpa-onnx-c-api.dylib');
  const stagedOnnxRuntime = path.join(prepared.runtimeLibDir, 'libonnxruntime.dylib');
  assert.deepEqual(
    commandCalls.filter((call) => call.executable === 'install_name_tool')
      .map(({ commandArgs }) => commandArgs)
      .sort((left, right) => left.join('\0').localeCompare(right.join('\0'))),
    [
      ['-id', '@rpath/libsherpa-onnx-c-api.dylib', stagedSherpa],
      ['-change', path.join(sourceRuntimeLibDir, 'libonnxruntime.dylib'), '@loader_path/libonnxruntime.dylib', stagedSherpa],
      ['-id', '@rpath/libonnxruntime.dylib', stagedOnnxRuntime],
    ].sort((left, right) => left.join('\0').localeCompare(right.join('\0'))),
  );
  const cargoCall = commandCalls.find((call) => call.executable === 'cargo');
  assert.deepEqual(cargoCall.commandArgs, ['build', '-p', 'sona-cli', '--release', '--target', target]);
  assert.equal(cargoCall.options.env.SHERPA_ONNX_LIB_DIR, prepared.runtimeLibDir);
});

test('desktop bundle preparer rejects FFmpeg checksum and native header mismatches', async () => {
  const { verifySha256, verifyNativeBinary } = await loadDesktopBundlePreparer();
  assert.equal(typeof verifySha256, 'function');
  assert.equal(typeof verifyNativeBinary, 'function');
  assert.throws(
    () => verifySha256(Buffer.from('ffmpeg'), '0'.repeat(64), 'https://example.invalid/ffmpeg'),
    /SHA-256 mismatch/u,
  );

  const pe = writeNativeFfmpegBinary('x86_64-pc-windows-msvc');
  assert.doesNotThrow(() => verifyNativeBinary(pe, 'x86_64-pc-windows-msvc', 'ffmpeg.exe'));
  const malformedPe = Buffer.from(pe);
  malformedPe.writeUInt16LE(0, 0x98);
  assert.throws(
    () => verifyNativeBinary(malformedPe, 'x86_64-pc-windows-msvc', 'ffmpeg.exe'),
    /PE executable/u,
  );
  assert.throws(
    () => verifyNativeBinary(pe, 'aarch64-pc-windows-msvc', 'ffmpeg.exe'),
    /aarch64-pc-windows-msvc PE executable/u,
  );

  const machO = writeNativeFfmpegBinary('aarch64-apple-darwin');
  assert.doesNotThrow(() => verifyNativeBinary(machO, 'aarch64-apple-darwin', 'ffmpeg'));
  assert.throws(
    () => verifyNativeBinary(Buffer.alloc(8), 'aarch64-apple-darwin', 'ffmpeg'),
    /not a 64-bit Mach-O executable/u,
  );
  const malformedMachO = Buffer.from(machO);
  malformedMachO.writeUInt32LE(0, 12);
  assert.throws(
    () => verifyNativeBinary(malformedMachO, 'aarch64-apple-darwin', 'ffmpeg'),
    /Mach-O executable/u,
  );
  assert.throws(
    () => verifyNativeBinary(machO, 'x86_64-apple-darwin', 'ffmpeg'),
    /architecture does not match/u,
  );

  const elf = writeNativeFfmpegBinary('x86_64-unknown-linux-gnu');
  assert.doesNotThrow(() => verifyNativeBinary(elf, 'x86_64-unknown-linux-gnu', 'ffmpeg'));
  assert.throws(
    () => verifyNativeBinary(Buffer.alloc(20), 'x86_64-unknown-linux-gnu', 'ffmpeg'),
    /not a x86_64-unknown-linux-gnu ELF executable/u,
  );
  const malformedElf = Buffer.from(elf);
  malformedElf.writeUInt16LE(0, 16);
  assert.throws(
    () => verifyNativeBinary(malformedElf, 'x86_64-unknown-linux-gnu', 'ffmpeg'),
    /ELF executable/u,
  );
  assert.throws(
    () => verifyNativeBinary(elf, 'aarch64-unknown-linux-gnu', 'ffmpeg'),
    /aarch64-unknown-linux-gnu ELF executable/u,
  );
});

test('desktop bundle preparer fails release preparation without SHERPA runtime anchors', async () => {
  const { prepareDesktopBundle } = await loadDesktopBundlePreparer();
  const root = makeTempRepo();
  const target = 'x86_64-pc-windows-msvc';

  await assert.rejects(
    prepareDesktopBundle({
      repoRoot: root,
      target,
      configPath: path.join(root, 'platforms', 'desktop', 'tauri.conf.json'),
      ffmpegLockPath: writeTestFfmpegLock(root, target),
      sherpaLibDir: null,
    }),
    /SHERPA_ONNX_LIB_DIR/u,
  );
});
