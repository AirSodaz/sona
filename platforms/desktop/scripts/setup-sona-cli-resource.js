import fs from 'fs';
import path from 'path';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const args = process.argv.slice(2);
const UNIVERSAL_MACOS_TARGET = 'universal-apple-darwin';
const UNIVERSAL_MACOS_SOURCE_TARGETS = ['aarch64-apple-darwin', 'x86_64-apple-darwin'];

function main() {
  const repoRoot = path.resolve(readFlagValue(args, '--repo-root') ?? path.resolve(__dirname, '../../..'));
  const target = resolveBuildTarget(args);
  const resourceDir = path.join(repoRoot, 'platforms', 'desktop', 'resources', 'cli');
  fs.mkdirSync(resourceDir, { recursive: true });

  cleanCliResource(resourceDir);

  if (target === UNIVERSAL_MACOS_TARGET) {
    stageUniversalMacosCli(repoRoot, resourceDir, args);
    return;
  }

  stageSingleTargetCli(repoRoot, target, resourceDir, args);
}

function stageSingleTargetCli(repoRoot, target, resourceDir, commandArgs) {
  const releaseDir = resolveReleaseDir(repoRoot, target, commandArgs);
  const binaryName = cliBinaryNameForTarget(target);
  const sourcePath = path.join(releaseDir, binaryName);

  assertSourceExists(repoRoot, target, sourcePath);

  const destinationPath = path.join(resourceDir, binaryName);
  fs.copyFileSync(sourcePath, destinationPath);
  makeExecutableIfNeeded(target, destinationPath);

  console.log(`[cli] Staged standalone CLI resource for ${target}: ${path.relative(repoRoot, destinationPath)}`);
}

function stageUniversalMacosCli(repoRoot, resourceDir, commandArgs) {
  const sourcePaths = UNIVERSAL_MACOS_SOURCE_TARGETS.map((sourceTarget) => {
    const releaseDir = resolveReleaseDir(repoRoot, sourceTarget, commandArgs);
    const sourcePath = path.join(releaseDir, cliBinaryNameForTarget(sourceTarget));
    assertSourceExists(repoRoot, sourceTarget, sourcePath);
    return sourcePath;
  });
  const destinationPath = path.join(resourceDir, 'sona-cli');

  if (process.platform !== 'darwin') {
    throw new Error('Building a universal macOS sona-cli resource requires lipo on macOS.');
  }

  const lipoResult = spawnSync('lipo', ['-create', ...sourcePaths, '-output', destinationPath], {
    cwd: repoRoot,
    stdio: 'inherit',
  });
  if (lipoResult.error) {
    throw lipoResult.error;
  }
  if (lipoResult.status !== 0) {
    throw new Error(`lipo failed while creating universal sona-cli resource with exit code ${lipoResult.status}`);
  }

  fs.chmodSync(destinationPath, 0o755);
  console.log(
    `[cli] Staged universal macOS standalone CLI resource: ${path.relative(repoRoot, destinationPath)}`,
  );
}

function cleanCliResource(resourceDir) {
  for (const staleName of ['sona-cli', 'sona-cli.exe']) {
    fs.rmSync(path.join(resourceDir, staleName), { force: true });
  }
}

function cliBinaryNameForTarget(target) {
  return target.includes('windows') ? 'sona-cli.exe' : 'sona-cli';
}

function assertSourceExists(repoRoot, target, sourcePath) {
  if (!fs.existsSync(sourcePath)) {
    throw new Error(`Missing standalone CLI binary for ${target}: ${path.relative(repoRoot, sourcePath)}`);
  }
}

function makeExecutableIfNeeded(target, destinationPath) {
  if (!target.includes('windows')) {
    fs.chmodSync(destinationPath, 0o755);
  }
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
  if (explicitReleaseDir && target !== UNIVERSAL_MACOS_TARGET) {
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

main();
