import fs from 'fs';
import path from 'path';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const srcTauriDir = path.resolve(repoRoot, 'src-tauri');
const args = process.argv.slice(2);

function main() {
  const target = resolveBuildTarget(args);
  const platform = getTargetPlatform(target);
  const stagedBinary = path.resolve(
    srcTauriDir,
    'binaries',
    `sona-cli-${target}${platform === 'windows' ? '.exe' : ''}`
  );
  const runtimeDir = path.resolve(srcTauriDir, 'binaries', 'cli-runtime');

  assertExists(stagedBinary, 'staged CLI sidecar');
  const runtimeFiles = readMatchingRuntimeFiles(runtimeDir, platform);
  if (runtimeFiles.length === 0) {
    throw new Error(`No staged runtime libraries found in ${runtimeDir}.`);
  }

  const bundledFiles = resolveBundleRoots(target).flatMap((bundleRoot) => walkFiles(bundleRoot));

  const bundledSidecars = bundledFiles.filter((filePath) =>
    path.basename(filePath).startsWith('sona-cli')
  );
  const bundledRuntimeFiles = bundledFiles.filter((filePath) =>
    getRuntimeFilePattern(platform).test(path.basename(filePath))
  );

  if (bundledSidecars.length === 0) {
    const opaqueArtifacts = bundledFiles.some((filePath) =>
      /\.(AppImage|deb|dmg|exe|msi|rpm)$/iu.test(filePath)
    );
    if (!opaqueArtifacts) {
      throw new Error('No bundled `sona-cli` binary was found under src-tauri/target/**/bundle.');
    }
  }

  if (bundledRuntimeFiles.length === 0) {
    const opaqueArtifacts = bundledFiles.some((filePath) =>
      /\.(AppImage|deb|dmg|exe|msi|rpm)$/iu.test(filePath)
    );
    if (!opaqueArtifacts) {
      throw new Error('No bundled runtime library was found under src-tauri/target/**/bundle.');
    }
  }

  console.log(`[cli-bundle] Verified staged assets for ${target}`);
  console.log(`[cli-bundle] Staged runtime libraries: ${runtimeFiles.map((filePath) => path.basename(filePath)).join(', ')}`);
  if (bundledSidecars.length > 0) {
    console.log(`[cli-bundle] Bundled sidecars: ${bundledSidecars.map((filePath) => path.relative(repoRoot, filePath)).join(', ')}`);
  } else {
    console.log('[cli-bundle] Bundled sidecar not directly inspectable; only opaque installer artifacts were found.');
  }
}

function resolveBuildTarget(commandArgs) {
  const explicitTarget = readFlagValue(commandArgs, '--target');
  if (explicitTarget) {
    return explicitTarget;
  }

  const rustcResult = spawnSync(resolveExecutable('rustc'), ['-vV'], {
    cwd: repoRoot,
    encoding: 'utf8',
    stdio: 'pipe',
  });
  if (rustcResult.error) {
    throw rustcResult.error;
  }

  const hostLine = rustcResult.stdout
    .split(/\r?\n/u)
    .find((line) => line.startsWith('host:'));
  if (!hostLine) {
    throw new Error('Unable to resolve the Rust host target triple for bundle verification.');
  }

  return hostLine.replace('host:', '').trim();
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

function resolveBundleRoots(target) {
  const candidateDirectories = [
    path.resolve(srcTauriDir, 'target', 'release', 'bundle'),
    path.resolve(srcTauriDir, 'target', target, 'release', 'bundle'),
  ];

  return [...new Set(candidateDirectories)].filter((directoryPath) =>
    fs.existsSync(directoryPath)
  );
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

function readMatchingRuntimeFiles(directoryPath, platform) {
  if (!fs.existsSync(directoryPath)) {
    return [];
  }

  const filePattern = getRuntimeFilePattern(platform);
  return fs
    .readdirSync(directoryPath, { withFileTypes: true })
    .filter((entry) => entry.isFile() && filePattern.test(entry.name))
    .map((entry) => path.resolve(directoryPath, entry.name));
}

function walkFiles(directoryPath) {
  const entries = fs.readdirSync(directoryPath, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const resolvedPath = path.resolve(directoryPath, entry.name);
    if (entry.isDirectory()) {
      return walkFiles(resolvedPath);
    }
    return [resolvedPath];
  });
}

function assertExists(filePath, label) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`Missing ${label}: ${filePath}`);
  }
}

function resolveExecutable(command) {
  const envOverrides = {
    rustc: process.env.RUSTC,
  };
  if (envOverrides[command]) {
    return envOverrides[command];
  }

  if (process.platform === 'win32' && command === 'rustc') {
    const candidate = path.resolve(
      process.env.USERPROFILE ?? '',
      '.cargo',
      'bin',
      'rustc.exe'
    );
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  return command;
}

main();
