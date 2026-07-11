import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { gunzipSync } from 'node:zlib';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
export function resolveHostTarget(platform = process.platform, arch = process.arch) {
  if (platform === 'win32') {
    return arch === 'arm64' ? 'aarch64-pc-windows-msvc' : 'x86_64-pc-windows-msvc';
  }
  if (platform === 'darwin') {
    return arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
  }
  if (platform === 'linux') {
    return arch === 'arm64' ? 'aarch64-unknown-linux-gnu' : 'x86_64-unknown-linux-gnu';
  }
  throw new Error(`Unsupported host platform for desktop bundle preparation: ${platform}/${arch}`);
}

export function assertSupportedTarget(target) {
  if (!target) {
    throw new Error('A desktop bundle target is required.');
  }
}

export async function prepareDesktopBundle({
  repoRoot,
  target = resolveHostTarget(),
  configPath = path.join(repoRoot, 'platforms', 'desktop', 'tauri.conf.json'),
  ffmpegLockPath = path.join(repoRoot, 'platforms', 'desktop', 'packaging', 'ffmpeg-sources.json'),
  sherpaLibDir = process.env.SHERPA_ONNX_LIB_DIR,
  runCommand = runRequired,
  readMacDylibDependencies = listMacDylibDependencies,
} = {}) {
  if (!repoRoot) {
    throw new Error('prepareDesktopBundle requires repoRoot.');
  }
  assertSupportedTarget(target);

  const stagingRoot = path.join(repoRoot, 'target', 'desktop-bundle', target);
  const sidecarsDir = path.join(stagingRoot, 'sidecars');
  const runtimeLibDir = path.join(stagingRoot, 'runtime-libs');
  fs.rmSync(stagingRoot, { recursive: true, force: true });
  fs.mkdirSync(sidecarsDir, { recursive: true });
  fs.mkdirSync(runtimeLibDir, { recursive: true });

  stageRuntimeLibraries(sherpaLibDir, target, runtimeLibDir);
  if (target.includes('apple')) {
    rebaseMacDylibs(runtimeLibDir, runCommand, readMacDylibDependencies);
  }
  buildStandaloneCli(repoRoot, target, runtimeLibDir, runCommand);
  stageCliSidecar(repoRoot, target, sidecarsDir);
  await stageFfmpegSidecar(target, sidecarsDir, ffmpegLockPath, stagingRoot, runCommand);

  const generatedConfigPath = path.join(stagingRoot, 'tauri.bundle.conf.json');
  writeBundleConfig(configPath, generatedConfigPath, sidecarsDir, runtimeLibDir, target);

  return {
    target,
    stagingRoot,
    sidecarsDir,
    runtimeLibDir,
    configPath: generatedConfigPath,
  };
}

function buildStandaloneCli(repoRoot, target, runtimeLibDir, runCommand) {
  const options = { cwd: repoRoot };
  if (target.includes('apple')) {
    options.env = { ...process.env, SHERPA_ONNX_LIB_DIR: runtimeLibDir };
  }
  runCommand('cargo', ['build', '-p', 'sona-cli', '--release', '--target', target], options);
}

function stageCliSidecar(repoRoot, target, sidecarsDir) {
  const sourcePath = path.join(repoRoot, 'target', target, 'release', cliBinaryName(target));
  if (!fs.existsSync(sourcePath)) {
    throw new Error(`Missing release sona-cli binary for ${target}: ${sourcePath}`);
  }
  const destinationPath = path.join(sidecarsDir, sidecarFileName('sona-cli', target));
  fs.copyFileSync(sourcePath, destinationPath);
  makeExecutableIfNeeded(target, destinationPath);
}

async function stageFfmpegSidecar(target, sidecarsDir, ffmpegLockPath, stagingRoot, runCommand) {
  const sources = JSON.parse(fs.readFileSync(ffmpegLockPath, 'utf8')).sources;
  const source = sources?.find((entry) => entry.target === target);
  if (!source) {
    throw new Error(`No FFmpeg source lock entry exists for ${target}.`);
  }

  const archive = await readDownload(source.url);
  verifySha256(archive, source.sha256, source.url);
  const extractionRoot = path.join(stagingRoot, 'ffmpeg-extract');
  fs.mkdirSync(extractionRoot, { recursive: true });
  const sourcePath = extractFfmpegBinary(archive, source, extractionRoot, runCommand);
  const binary = fs.readFileSync(sourcePath);
  verifyNativeBinary(binary, target, sourcePath);

  const destinationPath = path.join(sidecarsDir, sidecarFileName('ffmpeg', target));
  fs.copyFileSync(sourcePath, destinationPath);
  makeExecutableIfNeeded(target, destinationPath);
  fs.rmSync(extractionRoot, { recursive: true, force: true });
}

async function readDownload(url) {
  if (url.startsWith('file:')) {
    return fs.readFileSync(fileURLToPath(url));
  }
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Unable to download FFmpeg source ${url}: ${response.status} ${response.statusText}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

export function verifySha256(content, expectedHash, sourceUrl) {
  const actualHash = createHash('sha256').update(content).digest('hex');
  if (!/^[a-f0-9]{64}$/iu.test(expectedHash ?? '') || actualHash !== expectedHash.toLowerCase()) {
    throw new Error(`FFmpeg SHA-256 mismatch for ${sourceUrl}: expected ${expectedHash}, received ${actualHash}`);
  }
}

function extractFfmpegBinary(archive, source, extractionRoot, runCommand) {
  if (source.archiveKind === 'gz') {
    const binaryPath = path.join(extractionRoot, path.basename(source.binaryPath));
    fs.writeFileSync(binaryPath, gunzipSync(archive));
    return binaryPath;
  }

  const extension = source.archiveKind === 'zip' ? 'zip' : 'tar.xz';
  const archivePath = path.join(extractionRoot, `ffmpeg.${extension}`);
  fs.writeFileSync(archivePath, archive);
  if (source.archiveKind === 'zip') {
    extractZip(archivePath, extractionRoot, runCommand);
  } else if (source.archiveKind === 'tar.xz') {
    runCommand('tar', ['-xJf', archivePath, '-C', extractionRoot]);
  } else {
    throw new Error(`Unsupported FFmpeg archive kind: ${source.archiveKind}`);
  }

  const binaryPath = path.resolve(extractionRoot, source.binaryPath);
  if (!binaryPath.startsWith(`${extractionRoot}${path.sep}`) || !fs.existsSync(binaryPath)) {
    throw new Error(`FFmpeg archive did not contain the locked binary path: ${source.binaryPath}`);
  }
  return binaryPath;
}

function extractZip(archivePath, extractionRoot, runCommand) {
  if (process.platform === 'win32') {
    const command = `Expand-Archive -LiteralPath '${archivePath.replaceAll("'", "''")}' -DestinationPath '${extractionRoot.replaceAll("'", "''")}' -Force`;
    runCommand('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', command]);
    return;
  }
  runCommand('unzip', ['-q', archivePath, '-d', extractionRoot]);
}

export function verifyNativeBinary(binary, target, sourcePath) {
  if (target.includes('windows')) {
    verifyPe(binary, target, sourcePath);
    return;
  }
  if (target.includes('apple')) {
    verifyMachO(binary, target, sourcePath);
    return;
  }
  if (target.includes('linux')) {
    verifyElf(binary, target, sourcePath);
    return;
  }
  throw new Error(`Unsupported desktop bundle target: ${target}`);
}

function verifyPe(binary, target, sourcePath) {
  const peOffset = binary.length >= 0x40 ? binary.readUInt32LE(0x3c) : 0;
  const expectedMachine = target.includes('aarch64') ? 0xaa64 : 0x8664;
  const optionalHeaderStart = peOffset + 24;
  const optionalHeaderSize = peOffset + 22 <= binary.length ? binary.readUInt16LE(peOffset + 20) : 0;
  if (
    binary.subarray(0, 2).toString('ascii') !== 'MZ'
    || peOffset + 24 > binary.length
    || binary.subarray(peOffset, peOffset + 4).toString('ascii') !== 'PE\0\0'
    || binary.readUInt16LE(peOffset + 4) !== expectedMachine
    || optionalHeaderSize < 2
    || optionalHeaderStart + optionalHeaderSize > binary.length
    || binary.readUInt16LE(optionalHeaderStart) !== 0x20b
  ) {
    throw new Error(`FFmpeg binary is not a ${target} PE executable: ${sourcePath}`);
  }
}

function verifyMachO(binary, target, sourcePath) {
  if (binary.length < 32 || binary.readUInt32LE(0) !== 0xfeedfacf || binary.readUInt32LE(12) !== 2) {
    throw new Error(`FFmpeg binary is not a 64-bit Mach-O executable: ${sourcePath}`);
  }
  const expectedCpuType = target.includes('aarch64') ? 0x0100000c : 0x01000007;
  if (binary.readUInt32LE(4) !== expectedCpuType) {
    throw new Error(`FFmpeg binary architecture does not match ${target}: ${sourcePath}`);
  }
}

function verifyElf(binary, target, sourcePath) {
  const expectedMachine = target.includes('aarch64') ? 0xb7 : 0x3e;
  if (
    binary.length < 64
    || binary.subarray(0, 4).toString('latin1') !== '\x7fELF'
    || binary.readUInt8(4) !== 2
    || binary.readUInt8(5) !== 1
    || ![2, 3].includes(binary.readUInt16LE(16))
    || binary.readUInt16LE(18) !== expectedMachine
  ) {
    throw new Error(`FFmpeg binary is not a ${target} ELF executable: ${sourcePath}`);
  }
}

function stageRuntimeLibraries(sherpaLibDir, target, runtimeLibDir) {
  if (!sherpaLibDir) {
    throw new Error('SHERPA_ONNX_LIB_DIR is required for release desktop bundle preparation.');
  }
  if (!fs.existsSync(sherpaLibDir)) {
    throw new Error(`SHERPA_ONNX_LIB_DIR does not exist: ${sherpaLibDir}`);
  }

  const runtimeLibraries = fs.readdirSync(sherpaLibDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && isPlatformDynamicLibrary(entry.name, target));
  for (const library of runtimeLibraries) {
    fs.copyFileSync(path.join(sherpaLibDir, library.name), path.join(runtimeLibDir, library.name));
  }

  const stagedNames = new Set(runtimeLibraries.map((entry) => entry.name));
  const missingAnchors = requiredRuntimeLibraryAnchors(target)
    .filter((anchor) => ![...stagedNames].some((name) => typeof anchor === 'string' ? name === anchor : anchor.test(name)));
  if (missingAnchors.length > 0) {
    throw new Error(
      `SHERPA_ONNX_LIB_DIR is missing runtime library anchors for ${target}: ${missingAnchors.map(String).join(', ')}`,
    );
  }
}

function isPlatformDynamicLibrary(name, target) {
  const lowerName = name.toLowerCase();
  if (target.includes('windows')) return lowerName.endsWith('.dll');
  if (target.includes('apple')) return lowerName.includes('.dylib');
  if (target.includes('linux')) return lowerName.includes('.so');
  return false;
}

function requiredRuntimeLibraryAnchors(target) {
  if (target.includes('windows')) return ['sherpa-onnx-c-api.dll', 'onnxruntime.dll'];
  if (target.includes('apple')) return ['libsherpa-onnx-c-api.dylib', 'libonnxruntime.dylib'];
  if (target.includes('linux')) return [/^libsherpa-onnx-c-api\.so/u, /^libonnxruntime\.so/u];
  throw new Error(`Unsupported desktop bundle target: ${target}`);
}

function rebaseMacDylibs(runtimeLibDir, runCommand, readMacDylibDependencies) {
  const libraryNames = fs.readdirSync(runtimeLibDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.toLowerCase().includes('.dylib'))
    .map((entry) => entry.name);
  const bundledLibraries = new Set(libraryNames);

  for (const libraryName of libraryNames) {
    const libraryPath = path.join(runtimeLibDir, libraryName);
    runCommand('install_name_tool', ['-id', `@rpath/${libraryName}`, libraryPath]);

    // Framework-local dependencies must not retain paths from Sherpa's archive.
    for (const dependency of readMacDylibDependencies(libraryPath)) {
      const dependencyName = path.basename(dependency);
      if (dependencyName === libraryName || !bundledLibraries.has(dependencyName)) continue;
      runCommand('install_name_tool', [
        '-change',
        dependency,
        `@loader_path/${dependencyName}`,
        libraryPath,
      ]);
    }
  }
}

function listMacDylibDependencies(libraryPath) {
  const result = spawnSync('otool', ['-L', libraryPath], { encoding: 'utf8' });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`otool failed for ${libraryPath} with code ${result.status ?? 1}`);
  }
  return result.stdout
    .split(/\r?\n/u)
    .slice(1)
    .map((line) => line.trim().match(/^(.*?) \(compatibility version /u)?.[1])
    .filter(Boolean);
}

function writeBundleConfig(baseConfigPath, generatedConfigPath, sidecarsDir, runtimeLibDir, target) {
  const config = JSON.parse(fs.readFileSync(baseConfigPath, 'utf8'));
  config.bundle ??= {};
  clearGeneratedRuntimeMappings(config.bundle);
  if (target.includes('windows')) {
    config.bundle.resources = { [path.join(runtimeLibDir, '*')]: '' };
  } else if (target.includes('apple')) {
    config.bundle.macOS ??= {};
    config.bundle.macOS.files = runtimeLibraryFileMap(runtimeLibDir, 'Frameworks');
  } else if (target.includes('linux')) {
    const files = runtimeLibraryFileMap(runtimeLibDir, 'usr/lib/sona');
    config.bundle.linux ??= {};
    for (const format of ['deb', 'rpm', 'appimage']) {
      config.bundle.linux[format] ??= {};
      config.bundle.linux[format].files = files;
    }
  } else {
    throw new Error(`Unsupported desktop bundle target: ${target}`);
  }
  config.bundle.externalBin = [
    path.join(sidecarsDir, 'sona-cli'),
    path.join(sidecarsDir, 'ffmpeg'),
  ];
  fs.writeFileSync(generatedConfigPath, `${JSON.stringify(config, null, 2)}\n`);
}

function clearGeneratedRuntimeMappings(bundle) {
  delete bundle.resources;
  if (bundle.macOS) delete bundle.macOS.files;
  if (bundle.linux) {
    for (const format of ['deb', 'rpm', 'appimage']) {
      if (bundle.linux[format]) delete bundle.linux[format].files;
    }
  }
  for (const format of ['deb', 'rpm', 'appimage']) {
    if (bundle[format]) delete bundle[format].files;
  }
}

function runtimeLibraryFileMap(runtimeLibDir, destinationDir) {
  return Object.fromEntries(
    fs.readdirSync(runtimeLibDir, { withFileTypes: true })
      .filter((entry) => entry.isFile())
      .map((entry) => [
        path.posix.join(destinationDir, entry.name),
        path.join(runtimeLibDir, entry.name),
      ]),
  );
}

function cliBinaryName(target) {
  return target.includes('windows') ? 'sona-cli.exe' : 'sona-cli';
}

function sidecarFileName(binaryName, target) {
  return `${binaryName}-${target}${target.includes('windows') ? '.exe' : ''}`;
}

function makeExecutableIfNeeded(target, filePath) {
  if (!target.includes('windows')) {
    fs.chmodSync(filePath, 0o755);
  }
}

function runRequired(executable, commandArgs, options = {}) {
  const result = spawnSync(executable, commandArgs, {
    stdio: 'inherit',
    shell: process.platform === 'win32' && executable !== process.execPath,
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${executable} exited with code ${result.status ?? 1}`);
}

function readFlagValue(commandArgs, flagName) {
  for (let index = 0; index < commandArgs.length; index += 1) {
    const value = commandArgs[index];
    if (value === flagName) return commandArgs[index + 1] ?? null;
    if (value.startsWith(`${flagName}=`)) return value.slice(flagName.length + 1);
  }
  return null;
}

async function main() {
  const commandArgs = process.argv.slice(2);
  const repoRoot = path.resolve(readFlagValue(commandArgs, '--repo-root') ?? path.resolve(__dirname, '../../..'));
  const target = readFlagValue(commandArgs, '--target') ?? resolveHostTarget();
  const configPath = path.resolve(repoRoot, readFlagValue(commandArgs, '--config') ?? 'platforms/desktop/tauri.conf.json');
  const ffmpegLockPath = path.resolve(
    repoRoot,
    readFlagValue(commandArgs, '--ffmpeg-lock') ?? 'platforms/desktop/packaging/ffmpeg-sources.json',
  );
  const prepared = await prepareDesktopBundle({ repoRoot, target, configPath, ffmpegLockPath });
  console.log(`[bundle] Prepared ${prepared.target}: ${prepared.configPath}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  main().catch((error) => {
    console.error(`[bundle] ${error.message}`);
    process.exit(1);
  });
}
