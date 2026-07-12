import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { repoRoot, read, exists, desktopCrateSegments, desktopCratePath, desktopFrontendDependencies, rustFilesUnder, stripRustLineComment, readWorkflowStep, readWorkflowStepIndex, readWorkflowBuildTauriSteps } from './test-support/repository.js';
import { node, makeTempRepo, writeNativeFfmpegBinary, runtimeLibraryNames, writeRuntimeLibraries, runtimeFileMap, prepareBundleFixture, writeGeneratedBundleConfig, writeCanonicalAppBundle, writeTauriWrapperStubs, writeTestFfmpegLock, loadDesktopBundlePreparer } from './test-support/packaging-fixtures.js';

test('desktop bundle preparer derives supported host targets and maps runtime libraries', async () => {
  const { resolveHostTarget, assertSupportedTarget } = await loadDesktopBundlePreparer();

  assert.equal(resolveHostTarget('win32', 'x64'), 'x86_64-pc-windows-msvc');
  assert.equal(resolveHostTarget('win32', 'arm64'), 'aarch64-pc-windows-msvc');
  assert.equal(resolveHostTarget('darwin', 'arm64'), 'aarch64-apple-darwin');
  assert.equal(resolveHostTarget('linux', 'x64'), 'x86_64-unknown-linux-gnu');
});

test('desktop bundle wrapper keeps checked-in production inputs and resolves unknown targets from its source lock', async () => {
  const { prepareDesktopBundle } = await loadDesktopBundlePreparer();
  const target = 'x86_64-apple-darwin-unlisted';
  const root = makeTempRepo();
  const releaseDir = path.join(root, 'target', target, 'release');
  const runtimeLibDir = path.join(root, 'native-libs');
  const configPath = path.join(root, 'base-tauri.conf.json');
  const preparer = read(...desktopCrateSegments, 'scripts', 'prepare-desktop-bundle.js');
  const sourceLockPath = desktopCratePath('packaging', 'ffmpeg-sources.json');

  assert.equal(exists(...desktopCrateSegments, 'scripts', 'prepare-desktop-bundle.js'), true);
  assert.equal(exists(...desktopCrateSegments, 'packaging', 'ffmpeg-sources.json'), true);
  assert.doesNotMatch(preparer, /UNIVERSAL_MACOS_TARGET/u);

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

test('production FFmpeg lock entries contain complete operational metadata', () => {
  const lock = JSON.parse(read('platforms', 'desktop', 'packaging', 'ffmpeg-sources.json'));
  assert.deepEqual(
    lock.sources.map((entry) => entry.target).sort(),
    [
      'aarch64-apple-darwin',
      'aarch64-pc-windows-msvc',
      'aarch64-unknown-linux-gnu',
      'x86_64-apple-darwin',
      'x86_64-pc-windows-msvc',
      'x86_64-unknown-linux-gnu',
    ],
  );
  for (const entry of lock.sources) {
    assert.match(entry.url, /^https:\/\//u);
    assert.match(entry.sha256, /^[a-f0-9]{64}$/iu);
    assert.ok(entry.license);
    assert.ok(entry.archiveKind);
    assert.ok(entry.binaryPath);
  }

  const btbNSources = lock.sources.filter((entry) => entry.url.includes('BtbN/FFmpeg-Builds'));
  assert.equal(btbNSources.length, 4);
  for (const entry of btbNSources) {
    assert.match(entry.url, /\/releases\/download\/autobuild-\d{4}-\d{2}-\d{2}-\d{2}-\d{2}\//u);
    assert.doesNotMatch(entry.url, /\/latest\//u);
  }
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

test('desktop host is a direct platform crate with explicit CLI config', () => {
  const wrapper = read(...desktopCrateSegments, 'scripts', 'tauri.js');

  assert.equal(exists('platforms', 'desktop', 'Cargo.toml'), true);
  assert.equal(exists('src-tauri'), false);
  assert.match(wrapper, /const desktopTauriConfig = path\.join\(repoRoot, 'platforms', 'desktop', 'tauri\.conf\.json'\);/u);
  assert.match(wrapper, /function withDesktopConfig\(commandArgs\)/u);
  assert.match(wrapper, /return \[command, '--config', desktopTauriConfig, \.\.\.commandArgs\.slice\(1\)\];/u);
});

test('desktop frontend and Tauri configuration are colocated', () => {
  const frontend = (...segments) => path.join(repoRoot, 'platforms', 'desktop', 'frontend', ...segments);
  const desktopConfig = JSON.parse(read(...desktopCrateSegments, 'tauri.conf.json'));
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');
  const rootPackage = JSON.parse(read('package.json'));
  const frontendPackage = JSON.parse(fs.readFileSync(frontend('package.json'), 'utf8'));

  assert.equal(fs.existsSync(frontend('src', 'main.tsx')), true);
  assert.equal(fs.existsSync(frontend('public', 'audio-processor.js')), true);
  assert.equal(fs.existsSync(frontend('vite.config.ts')), true);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'public')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'index.html')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'vite.config.ts')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'eslint.config.js')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'tsconfig.json')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'tsconfig.node.json')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'playwright.config.ts')), false);
  assert.equal(fs.existsSync(frontend('index.html')), true);
  assert.equal(desktopConfig.build.frontendDist, 'frontend/dist');
  assert.equal(desktopConfig.bundle.resources, undefined);
  assert.equal(desktopConfig.bundle.externalBin, undefined);
  for (const command of [desktopConfig.build.beforeDevCommand, desktopConfig.build.beforeBuildCommand]) {
    assert.equal(typeof command, 'object');
    assert.equal(typeof command.script, 'string');
    assert.equal(command.cwd, 'frontend');
  }
  assert.match(desktopLib, /"frontend\/src\/bindings\.ts"/u);
  assert.ok(frontendPackage.dependencies['@tauri-apps/api']);
  for (const dependencyName of desktopFrontendDependencies) {
    assert.equal(
      rootPackage.dependencies?.[dependencyName],
      undefined,
      `${dependencyName} must not be a root production dependency`,
    );
    assert.equal(
      rootPackage.devDependencies?.[dependencyName],
      undefined,
      `${dependencyName} must not be a root development dependency`,
    );
  }
  assert.equal(rootPackage.dependencies?.ws, undefined, 'root ws dependency has no remaining consumer');
  assert.equal(rootPackage.devDependencies?.ws, undefined, 'root ws dev dependency has no remaining consumer');
  assert.equal(rootPackage.scripts.tauri, 'node platforms/desktop/scripts/tauri.js');
  assert.equal(exists('platforms', 'desktop', 'scripts', 'tauri.js'), true);
  assert.equal(exists('scripts', 'tauri.js'), false);
});

test('desktop tauri crate no longer bundles sona-cli sidecar artifacts', () => {
  const libRs = read(...desktopCrateSegments, 'src', 'lib.rs');
  const cargoToml = read(...desktopCrateSegments, 'Cargo.toml');
  const tauriConfig = read(...desktopCrateSegments, 'tauri.conf.json');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );
  const tauriScript = read(...desktopCrateSegments, 'scripts', 'tauri.js');
  const oldCliSidecarScript = ['prepare', 'cli', 'sidecar'].join('-');
  const oldCliBundleScript = ['verify', 'cli', 'bundle'].join('-');

  assert.equal(exists(...desktopCrateSegments, 'src', 'cli'), false);
  assert.doesNotMatch(cargoToml, /^clap\s*=/mu);
  assert.doesNotMatch(cargoToml, /^clap_complete\s*=/mu);
  assert.doesNotMatch(tauriConfig, /binaries\/sona-cli/u);
  assert.doesNotMatch(tauriScript, new RegExp(oldCliSidecarScript, 'u'));
  assert.doesNotMatch(prWorkflow, new RegExp(`${oldCliSidecarScript}|${oldCliBundleScript}`, 'u'));

  const desktopCliCoreReferences = rustFilesUnder(desktopCratePath('src'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /sona_core::cli_|OfflineTranscribeCliOptions/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));

  assert.deepEqual(desktopCliCoreReferences, []);
});

test('packaging cleanup removes legacy source-tree resources and the ffmpeg-static package', () => {
  const frontendPackage = JSON.parse(read('platforms', 'desktop', 'frontend', 'package.json'));
  const lockfile = read('pnpm-lock.yaml');
  const workspaceConfig = read('pnpm-workspace.yaml');
  const rootIgnore = read('.gitignore');
  const desktopIgnore = read(...desktopCrateSegments, '.gitignore');
  const prWorkflow = read('.github', 'workflows', 'pr-guardrails.yml');

  assert.equal(frontendPackage.dependencies?.['ffmpeg-static'], undefined);
  assert.equal(frontendPackage.devDependencies?.['ffmpeg-static'], undefined);
  assert.doesNotMatch(lockfile, /^\s+ffmpeg-static:/mu);
  assert.doesNotMatch(lockfile, /^\s+ffmpeg-static@/mu);
  assert.doesNotMatch(workspaceConfig, /ffmpeg-static/u);
  assert.equal(exists(...desktopCrateSegments, 'scripts', 'setup-ffmpeg.js'), false);
  assert.equal(exists(...desktopCrateSegments, 'scripts', 'setup-sona-cli-resource.js'), false);
  assert.equal(exists(...desktopCrateSegments, 'resources', 'cli', '.gitkeep'), false);
  assert.doesNotMatch(rootIgnore, /platforms\/desktop\/(?:resources\/(?:shared_libs|cli)|binaries)\//u);
  assert.doesNotMatch(desktopIgnore, /^\/(?:binaries|models)$/mu);
  assert.match(desktopIgnore, /^\/target\/?$/mu);
  assert.doesNotMatch(prWorkflow, /setup-ffmpeg|setup-sona-cli-resource/u);
});

test('tauri wrapper passes generated config to build and bundle while dev preserves its base config', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-tauri-wrapper-'));
  const target = 'test-wrapper-target';
  const customDevConfig = path.join(root, 'dev-tauri.conf.json');
  const { logPath, preparerPath, tauriBinary } = writeTauriWrapperStubs(root);
  fs.writeFileSync(customDevConfig, '{}');

  const run = (command, commandArgs = []) => {
    fs.rmSync(logPath, { force: true });
    const result = spawnSync(
      node,
      [path.join(repoRoot, 'platforms', 'desktop', 'scripts', 'tauri.js'), command, ...commandArgs],
      {
        cwd: repoRoot,
        encoding: 'utf8',
        env: {
          ...process.env,
          SONA_TAURI_BINARY: tauriBinary,
          SONA_DESKTOP_BUNDLE_PREPARER: preparerPath,
          SONA_TAURI_ARGS_LOG: logPath,
          SHERPA_ONNX_LIB_DIR: path.join(root, 'source-runtime-libs'),
        },
      },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);
    return JSON.parse(fs.readFileSync(logPath, 'utf8'));
  };

  const generatedConfig = path.join(
    repoRoot,
    'target',
    'desktop-bundle',
    target,
    'tauri.bundle.conf.json',
  );
  const macTarget = 'aarch64-apple-darwin';
  t.after(() => fs.rmSync(path.join(repoRoot, 'target', 'desktop-bundle', target), { recursive: true, force: true }));
  t.after(() => fs.rmSync(path.join(repoRoot, 'target', 'desktop-bundle', macTarget), { recursive: true, force: true }));
  for (const command of ['build', 'bundle']) {
    const invocation = run(command, ['--target', target]);
    assert.deepEqual(invocation.args.slice(0, 3), [command, '--config', generatedConfig]);
  }
  const devInvocation = run('dev', ['--config', customDevConfig, '--help']);
  assert.deepEqual(devInvocation.args.slice(0, 3), ['dev', '--config', customDevConfig]);
  const macInvocation = run('build', ['--target', macTarget]);
  assert.equal(
    macInvocation.sherpaLibDir,
    path.join(repoRoot, 'target', 'desktop-bundle', macTarget, 'runtime-libs'),
  );
});

test('native runtime linking uses bundle-native loader contracts without runtime path mutation', () => {
  const cliCargo = read('platforms', 'cli', 'Cargo.toml');
  const cliBuild = read('platforms', 'cli', 'build.rs');
  const cliMain = read('platforms', 'cli', 'src', 'main.rs');
  const runtimeFsCargo = read('adapters', 'runtime_fs', 'Cargo.toml');
  const runtimeFsLib = read('adapters', 'runtime_fs', 'src', 'lib.rs');
  const desktopBuild = read(...desktopCrateSegments, 'build.rs');
  const desktopCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');

  const rpaths = (buildScript) => [
    ...buildScript.matchAll(/cargo:rustc-link-arg=-Wl,-rpath,([^"]+)/gu),
  ].map(([, rpath]) => rpath);

  assert.match(cliCargo, /^build\s*=\s*"build\.rs"/mu);
  assert.match(cliBuild, /SHERPA_ONNX_LIB_DIR/u);
  assert.match(cliBuild, /delayimp\.lib/u);
  assert.match(cliBuild, /\/DELAYLOAD:sherpa-onnx-c-api\.dll/u);
  assert.match(desktopBuild, /SHERPA_ONNX_LIB_DIR/u);
  assert.match(desktopBuild, /sherpa_lib_dir_has_directml/u);
  assert.match(desktopBuild, /delayimp\.lib/u);
  assert.match(desktopBuild, /\/DELAYLOAD:sherpa-onnx-c-api\.dll/u);
  assert.deepEqual(rpaths(desktopBuild), ['$ORIGIN/../lib/sona', '@loader_path/../Frameworks']);
  assert.deepEqual(rpaths(cliBuild), ['$ORIGIN/../lib/sona', '@loader_path/../Frameworks']);
  assert.doesNotMatch(
    desktopBuild,
    /resources\/shared_libs|copy_shared_libs|copy_from_target_dir|find_target_dir|create_dir_all|std::fs::write/u,
  );
  assert.doesNotMatch(runtimeFsLib, /shared_library_directory|SetDllDirectoryW|PCWSTR|OsStrExt/u);
  assert.doesNotMatch(cliMain, /init_cli_shared_library_directory|SetDllDirectoryW|PCWSTR|OsStrExt/u);
  assert.doesNotMatch(
    desktopLib,
    /init_dll_directory|SetDllDirectoryW|PCWSTR|OsStrExt|windows-test-manifest/u,
  );
  assert.doesNotMatch(runtimeFsCargo, /\bwindows\b|Win32_System_LibraryLoader/u);
  assert.doesNotMatch(desktopCargo, /Win32_System_LibraryLoader/u);
  assert.doesNotMatch(cliMain, /tauri/u);
});

test('standalone CLI keeps local ASR implementation behind its adapter boundary', () => {
  const cliSrcRoot = path.join(repoRoot, 'platforms', 'cli', 'src');
  const adapterPath = path.join(cliSrcRoot, 'asr_adapter.rs');
  const cliLib = fs.readFileSync(path.join(cliSrcRoot, 'lib.rs'), 'utf8');
  const adapter = fs.existsSync(adapterPath) ? fs.readFileSync(adapterPath, 'utf8') : '';
  const localAsrReferencesOutsideAdapter = rustFilesUnder(cliSrcRoot)
    .filter((filePath) => filePath !== adapterPath)
    .flatMap((filePath) => {
      const relativePath = path.relative(repoRoot, filePath);
      return fs.readFileSync(filePath, 'utf8')
        .split(/\r?\n/u)
        .flatMap((line, index) => {
          const sourceLine = stripRustLineComment(line);
          return /sona_local_asr::|LocalBatchAsrAdapter|sherpa_onnx::/u.test(sourceLine)
            ? [`${relativePath}:${index + 1}: ${line.trim()}`]
            : [];
        });
    });

  assert.deepEqual(localAsrReferencesOutsideAdapter, []);
  assert.match(cliLib, /\bmod asr_adapter;/u);
  assert.match(adapter, /sona_local_asr::batch::LocalBatchAsrAdapter/u);
});

test('release workflows invoke the bundle preparer without source-tree staging', () => {
  assert.equal(exists('scripts', 'package-sona-cli.js'), false);

  for (const workflowName of ['release.yml', 'nightly.yml']) {
    const workflow = read('.github', 'workflows', workflowName);

    assert.match(workflow, /args:\s*"--target aarch64-apple-darwin"/u);
    assert.match(workflow, /args:\s*"--target x86_64-apple-darwin"/u);
    assert.match(workflow, /node platforms\/desktop\/scripts\/tauri\.js build \$\{\{ matrix\.args \}\}/u);
    assert.doesNotMatch(workflow, /cargo build -p sona-cli|setup-sona-cli-resource|SONA_SKIP_CLI_RESOURCE_PREP/u);
    assert.doesNotMatch(workflow, /\blipo\b/u);
    assert.doesNotMatch(workflow, /node scripts\/package-sona-cli\.js/u);
    assert.doesNotMatch(workflow, /target\/\*\*\/release\/sona-cli-\*\.tar\.gz/u);
  }
});

test('release workflow build steps defer sidecar preparation to the Tauri wrapper', () => {
  for (const workflowName of ['release.yml', 'nightly.yml']) {
    const buildAppStep = readWorkflowStep(workflowName, 'Build the app');
    const verifyBundleStep = readWorkflowStep(workflowName, 'Verify Tauri bundle and shared libraries');
    const buildSteps = readWorkflowBuildTauriSteps(workflowName);

    assert.equal(buildSteps.some((step) => /standalone CLI|Stage .*resource/u.test(step.name ?? '')), false);
    assert.equal(buildAppStep.env.LD_LIBRARY_PATH, '${{ env.SHERPA_ONNX_LIB_DIR }}');
    assert.equal(buildAppStep.env.SONA_SKIP_CLI_RESOURCE_PREP, undefined);
    assert.equal(buildAppStep.run, 'node platforms/desktop/scripts/tauri.js build ${{ matrix.args }}');
    assert.equal(verifyBundleStep.run, 'node platforms/desktop/scripts/verify-tauri-bundle.js ${{ matrix.args }}');
    assert.ok(readWorkflowStepIndex(workflowName, 'Build the app') < readWorkflowStepIndex(workflowName, 'Verify Tauri bundle and shared libraries'));
  }
});

test('CLI documentation describes standalone sona-cli packaging only', () => {
  const readme = read('README.md');
  const readmeZh = read('README.zh-CN.md');
  const cliGuide = read('docs', 'cli.md');
  const cliGuideZh = read('docs', 'cli.zh-CN.md');
  const apiGuide = read('docs', 'api.md');
  const apiGuideZh = read('docs', 'api.zh-CN.md');
  const docs = `${readme}\n${readmeZh}\n${cliGuide}\n${cliGuideZh}\n${apiGuide}\n${apiGuideZh}`;

  assert.match(readme, /cargo run -p sona-cli -- transcribe/u);
  assert.match(readmeZh, /cargo run -p sona-cli -- transcribe/u);
  assert.match(readme, /pnpm run build:sona-cli/u);
  assert.match(readmeZh, /pnpm run build:sona-cli/u);
  assert.match(cliGuide, /### `transcribe`/u);
  assert.match(cliGuide, /sona-cli transcribe/u);

  assert.doesNotMatch(docs, /main desktop executable/u);
  assert.doesNotMatch(docs, /Sona\.exe transcribe/u);
  assert.doesNotMatch(docs, /Contents\/MacOS\/Sona transcribe/u);
  assert.doesNotMatch(docs, /cargo run --manifest-path src-tauri\/Cargo\.toml/u);
  assert.doesNotMatch(docs, /not part of the current standalone surface yet/u);
  assert.doesNotMatch(docs, /\bsona serve\b/u);
  assert.match(docs, /\bsona-cli serve\b/u);
});
