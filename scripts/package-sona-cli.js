import { spawnSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const args = process.argv.slice(2);

function main() {
  const repoRoot = path.resolve(readFlagValue(args, '--repo-root') ?? path.resolve(__dirname, '..'));
  const target = resolveBuildTarget(args);
  const releaseDir = resolveReleaseDir(repoRoot, target, args);
  const libDir = path.resolve(
    repoRoot,
    readFlagValue(args, '--lib-dir') ?? process.env.SHERPA_ONNX_LIB_DIR ?? '',
  );

  if (!fs.existsSync(libDir)) {
    throw new Error(`Missing sherpa-onnx shared library directory: ${libDir}`);
  }

  const binaryName = target.includes('windows') ? 'sona-cli.exe' : 'sona-cli';
  const binaryPath = path.join(releaseDir, binaryName);
  if (!fs.existsSync(binaryPath)) {
    throw new Error(`Missing standalone CLI binary for ${target}: ${path.relative(repoRoot, binaryPath)}`);
  }

  const packageName = `sona-cli-${target}`;
  const packageRoot = path.join(releaseDir, 'sona-cli-package');
  const packageDir = path.join(packageRoot, packageName);
  fs.rmSync(packageDir, { recursive: true, force: true });
  fs.mkdirSync(packageDir, { recursive: true });

  fs.copyFileSync(binaryPath, path.join(packageDir, binaryName));
  for (const libraryPath of resolveRequiredSharedLibraries(libDir, target)) {
    fs.copyFileSync(libraryPath, path.join(packageDir, path.basename(libraryPath)));
  }

  const archivePath = path.join(releaseDir, `${packageName}.tar.gz`);
  fs.rmSync(archivePath, { force: true });
  createTarGzArchive(packageRoot, packageName, archivePath);

  console.log(`[cli] Packaged standalone CLI for ${target}: ${path.relative(repoRoot, archivePath)}`);
}

function resolveBuildTarget(commandArgs) {
  const explicitTarget = readFlagValue(commandArgs, '--target');
  if (explicitTarget) {
    return explicitTarget;
  }

  if (process.platform === 'win32') {
    return 'x86_64-pc-windows-msvc';
  }
  if (process.platform === 'darwin') {
    return 'aarch64-apple-darwin';
  }
  return 'x86_64-unknown-linux-gnu';
}

function resolveReleaseDir(repoRoot, target, commandArgs) {
  const explicitReleaseDir = readFlagValue(commandArgs, '--release-dir');
  if (explicitReleaseDir) {
    return path.resolve(repoRoot, explicitReleaseDir);
  }

  return readFlagValue(commandArgs, '--target')
    ? path.resolve(repoRoot, 'target', target, 'release')
    : path.resolve(repoRoot, 'target', 'release');
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

function resolveRequiredSharedLibraries(libDir, target) {
  const entries = fs.existsSync(libDir) ? fs.readdirSync(libDir) : [];
  return requiredSharedLibraries(target).map((library) => {
    const filename =
      typeof library === 'string'
        ? entries.find((entry) => entry === library)
        : entries.find((entry) => library.test(entry));

    if (!filename) {
      throw new Error(
        `Missing shared library for ${target}: ${String(library)}. Found: ${entries.join(', ')}`,
      );
    }

    return path.join(libDir, filename);
  });
}

function requiredSharedLibraries(target) {
  if (target.includes('windows')) {
    return ['sherpa-onnx-c-api.dll', 'onnxruntime.dll'];
  }
  if (target.includes('apple')) {
    return ['libsherpa-onnx-c-api.dylib', 'libonnxruntime.dylib'];
  }
  return [/^libsherpa-onnx-c-api\.so/, /^libonnxruntime\.so/];
}

function createTarGzArchive(packageRoot, packageName, archivePath) {
  const result = spawnSync('tar', ['-czf', archivePath, '-C', packageRoot, packageName], {
    encoding: 'utf8',
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || `tar exited with status ${result.status}`);
  }
}

main();
