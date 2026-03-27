import fs from 'fs';
import path from 'path';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const srcTauriDir = path.resolve(repoRoot, 'src-tauri');
const binariesDir = path.resolve(srcTauriDir, 'binaries');
const runtimeDir = path.resolve(binariesDir, 'cli-runtime');
const args = process.argv.slice(2);

const DEFAULT_MACOS_ARCHS = ['aarch64-apple-darwin', 'x86_64-apple-darwin'];

function main() {
  const target = resolveBuildTarget(args);
  const profile = isDebugBuild(args) ? 'debug' : 'release';

  fs.mkdirSync(binariesDir, { recursive: true });
  resetDirectory(runtimeDir);

  const builtBinary = buildCliBinary(target, profile);
  const stagedBinary = stageCliBinary(target, builtBinary);
  patchRuntimeLookup(target, stagedBinary);
  stageRuntimeLibraries(target, profile);

  console.log(`[cli-sidecar] Prepared ${path.basename(stagedBinary)} for ${target}`);
}

function isDebugBuild(commandArgs) {
  return commandArgs.includes('--debug');
}

function resolveBuildTarget(commandArgs) {
  const explicitTarget = readFlagValue(commandArgs, '--target');
  if (explicitTarget) {
    return explicitTarget;
  }

  return resolveHostTarget();
}

function readFlagValue(commandArgs, flagName) {
  for (let index = 0; index < commandArgs.length; index += 1) {
    const value = commandArgs[index];
    if (value === flagName) {
      return commandArgs[index + 1] ?? null;
    }
    if (value.startsWith(`${flagName}=`)) {
      return value.slice(flagName.length + 1);
    }
  }
  return null;
}

function resolveHostTarget() {
  const rustcResult = runCommand('rustc', ['-vV'], { captureOutput: true });
  const hostLine = rustcResult.stdout
    .split(/\r?\n/u)
    .find((line) => line.startsWith('host:'));

  if (!hostLine) {
    throw new Error('Unable to resolve the Rust host target triple from `rustc -vV`.');
  }

  return hostLine.replace('host:', '').trim();
}

function buildCliBinary(target, profile) {
  if (target === 'universal-apple-darwin') {
    return buildUniversalMacOsBinary(profile);
  }

  return buildSingleTargetBinary(target, profile);
}

function buildUniversalMacOsBinary(profile) {
  const archBinaries = DEFAULT_MACOS_ARCHS.map((target) =>
    buildSingleTargetBinary(target, profile)
  );
  const universalOutputDir = path.resolve(srcTauriDir, 'target', 'cli-sidecar');
  const universalBinary = path.resolve(
    universalOutputDir,
    `sona-cli-universal${getBinaryExtension('macos')}`
  );

  fs.mkdirSync(universalOutputDir, { recursive: true });

  runCommand('lipo', ['-create', '-output', universalBinary, ...archBinaries]);

  return universalBinary;
}

function buildSingleTargetBinary(target, profile) {
  const platform = getTargetPlatform(target);
  const expectedBinaryPath = path.resolve(
    binariesDir,
    `sona-cli-${target}${getBinaryExtension(platform)}`
  );

  // Provide a dummy binary to break tauri-build cyclic dependency
  if (!fs.existsSync(expectedBinaryPath)) {
    fs.mkdirSync(path.dirname(expectedBinaryPath), { recursive: true });
    fs.writeFileSync(expectedBinaryPath, '');
  }

  const cargoArgs = [
    'build',
    '--manifest-path',
    path.join('src-tauri', 'Cargo.toml'),
    '--bin',
    'sona-cli',
    '--target',
    target,
  ];

  if (profile === 'release') {
    cargoArgs.push('--release');
  }

  runCommand('cargo', cargoArgs);

  return path.resolve(
    srcTauriDir,
    'target',
    target,
    profile,
    `sona-cli${getBinaryExtension(platform)}`
  );
}

function stageCliBinary(target, sourcePath) {
  const platform = getTargetPlatform(target);
  const destinationPath = path.resolve(
    binariesDir,
    `sona-cli-${target}${getBinaryExtension(platform)}`
  );

  fs.copyFileSync(sourcePath, destinationPath);
  if (platform !== 'windows') {
    fs.chmodSync(destinationPath, 0o755);
  }

  return destinationPath;
}

function patchRuntimeLookup(target, binaryPath) {
  const platform = getTargetPlatform(target);
  if (platform === 'linux') {
    runCommand('patchelf', ['--set-rpath', '$ORIGIN', binaryPath]);
    return;
  }

  if (platform !== 'macos') {
    return;
  }

  const libDir = process.env.SHERPA_ONNX_LIB_DIR;
  if (!libDir) {
    return;
  }

  const linkedLibraries = runCommand('otool', ['-L', binaryPath], {
    captureOutput: true,
  }).stdout;

  const linkedPaths = linkedLibraries
    .split(/\r?\n/u)
    .slice(1)
    .map((line) => line.trim().split(' ')[0])
    .filter(Boolean);

  for (const linkedPath of linkedPaths) {
    if (!linkedPath.startsWith(libDir)) {
      continue;
    }

    const replacementPath = `@executable_path/${path.basename(linkedPath)}`;
    runCommand('install_name_tool', ['-change', linkedPath, replacementPath, binaryPath]);
  }

  runCommand('install_name_tool', ['-delete_rpath', libDir, binaryPath], {
    allowFailure: true,
  });
  runCommand('install_name_tool', ['-add_rpath', '@executable_path', binaryPath]);
}

function stageRuntimeLibraries(target, profile) {
  const platform = getTargetPlatform(target);
  const builtBinaryDirectory = path.dirname(
    path.resolve(
      srcTauriDir,
      'target',
      target === 'universal-apple-darwin' ? DEFAULT_MACOS_ARCHS[0] : target,
      profile,
      `sona-cli${getBinaryExtension(platform)}`
    )
  );
  const searchDirectories = getRuntimeSearchDirectories(builtBinaryDirectory);
  const runtimeLibraries = collectRuntimeLibraries(platform, searchDirectories);

  if (runtimeLibraries.length === 0) {
    throw new Error(
      `Unable to find runtime libraries for ${target}. Set SHERPA_ONNX_LIB_DIR or provide the runtime files in ${srcTauriDir}.`
    );
  }

  for (const sourcePath of runtimeLibraries) {
    const destinationPath = path.resolve(runtimeDir, path.basename(sourcePath));
    fs.copyFileSync(sourcePath, destinationPath);
  }
}

function getRuntimeSearchDirectories(builtBinaryDirectory) {
  const directories = [
    process.env.SHERPA_ONNX_LIB_DIR,
    builtBinaryDirectory,
    srcTauriDir,
  ].filter(Boolean);

  return [...new Set(directories)].filter((directoryPath) => fs.existsSync(directoryPath));
}

function collectRuntimeLibraries(platform, directories) {
  const runtimeFilePattern = getRuntimeFilePattern(platform);
  const runtimeFiles = new Map();

  for (const directoryPath of directories) {
    const entries = fs.readdirSync(directoryPath, { withFileTypes: true });
    for (const entry of entries) {
      if (!entry.isFile() || !runtimeFilePattern.test(entry.name)) {
        continue;
      }

      if (!runtimeFiles.has(entry.name)) {
        runtimeFiles.set(entry.name, path.resolve(directoryPath, entry.name));
      }
    }
  }

  return [...runtimeFiles.values()];
}

function getRuntimeFilePattern(platform) {
  if (platform === 'windows') {
    return /^(sherpa-onnx|onnxruntime).*\.dll$/iu;
  }

  if (platform === 'macos') {
    return /^(lib)?(sherpa-onnx|onnxruntime).*\.(dylib)$/iu;
  }

  return /^(lib)?(sherpa-onnx|onnxruntime).*\.(so(\..*)?)$/iu;
}

function getBinaryExtension(platform) {
  return platform === 'windows' ? '.exe' : '';
}

function getTargetPlatform(target) {
  if (target.includes('windows')) {
    return 'windows';
  }
  if (target.includes('apple') || target.includes('darwin')) {
    return 'macos';
  }
  if (target.includes('linux')) {
    return 'linux';
  }

  throw new Error(`Unsupported target platform: ${target}`);
}

function resetDirectory(directoryPath) {
  fs.rmSync(directoryPath, { recursive: true, force: true });
  fs.mkdirSync(directoryPath, { recursive: true });
}

function runCommand(command, commandArgs, options = {}) {
  const { allowFailure = false, captureOutput = false } = options;
  const result = spawnSync(resolveExecutable(command), commandArgs, {
    cwd: repoRoot,
    encoding: 'utf8',
    stdio: captureOutput ? 'pipe' : 'inherit',
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0 && !allowFailure) {
    throw new Error(`Command failed: ${command} ${commandArgs.join(' ')}`);
  }

  return result;
}

function resolveExecutable(command) {
  const envOverrides = {
    cargo: process.env.CARGO,
    rustc: process.env.RUSTC,
  };
  if (envOverrides[command]) {
    return envOverrides[command];
  }

  if (process.platform === 'win32' && (command === 'cargo' || command === 'rustc')) {
    const candidate = path.resolve(
      process.env.USERPROFILE ?? '',
      '.cargo',
      'bin',
      `${command}.exe`
    );
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  return command;
}

main();
