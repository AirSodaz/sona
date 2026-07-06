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
  const binaryName = target.includes('windows') ? 'sona-cli.exe' : 'sona-cli';
  const sourcePath = path.join(releaseDir, binaryName);

  if (!fs.existsSync(sourcePath)) {
    throw new Error(`Missing standalone CLI binary for ${target}: ${path.relative(repoRoot, sourcePath)}`);
  }

  const resourceDir = path.join(repoRoot, 'src-tauri', 'resources', 'cli');
  fs.mkdirSync(resourceDir, { recursive: true });
  for (const staleName of ['sona-cli', 'sona-cli.exe']) {
    fs.rmSync(path.join(resourceDir, staleName), { force: true });
  }

  const destinationPath = path.join(resourceDir, binaryName);
  fs.copyFileSync(sourcePath, destinationPath);
  if (!target.includes('windows')) {
    fs.chmodSync(destinationPath, 0o755);
  }

  console.log(`[cli] Staged standalone CLI resource for ${target}: ${path.relative(repoRoot, destinationPath)}`);
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

main();
